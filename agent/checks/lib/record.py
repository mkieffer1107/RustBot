"""Records the oracle's behaviour over a corpus into goldens (one shard at a time) and merges the shards.

    python record.py shard --corpus C --out DIR --shard i/N [--sample PCT] [--extras-every K]
    python record.py merge --corpus C --out DIR --shards N [--sample PCT]

Shards are contiguous blocks of the (sampled) corpus, so concatenating shard outputs in order gives the same bytes for
any N. Each conversation runs in a forked child of a loaded PyBot (see oracle.py).
"""
import argparse
import json
import os
import pickle
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import oracle  # noqa: E402

oracle.ensure_env()
from lib import canon, paths, probes, pyrt_cases, sampling  # noqa: E402
from lib.paths import dump_line, open_text, read_jsonl  # noqa: E402

FUNC_CAP, RE_CAP, RE_PAT_CAP = 200, 3, 6


def select(corpus, pct):
    return [c for c in read_jsonl(corpus) if sampling.in_sample(c["id"], pct)]


def block(convs, i, n):
    size = -(-len(convs) // n)
    return convs[i * size:(i + 1) * size]


# ------------------------------------------------------------------ one conversation (runs in a forked child)

def run_conversation(conv, extras):
    ctx = {"conv": conv["id"], "turn": 0, "seed": conv["seed"], "clock": conv["clock"]}
    hooks = []
    oracle.install_recorders(lambda m, h, rec: hooks.append((m, h, rec)), ctx)
    if extras:
        probes.install_function_recorder(ctx)
        probes.install_call_probe()
        probes.ReLog.enabled = True
        oracle.RngState.log = []
    memory, turns = {}, []
    for i, text in enumerate(conv["turns"]):
        ctx["turn"] = i
        result = oracle.run_turn(text, memory)
        turns.append(oracle.turn_record(conv, i, text, result, memory))
    out = {"e2e": turns, "hooks": hooks}
    if extras:
        out.update(functions=probes.FuncLog.records, impure=sorted(probes.FuncLog.impure), re=probes.ReLog.cases,
                   strs=probes.CallLog.str_cases, floats=probes.CallLog.floats, trace=probes.CallLog.trace,
                   random=oracle.RngState.log)
    return out


def fork_run(fn):
    """Run fn() in a child process; return its pickled result."""
    r, w = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(r)
        try:
            data = pickle.dumps(("ok", fn()), protocol=4)
        except BaseException as e:  # noqa: BLE001
            import traceback
            data = pickle.dumps(("err", traceback.format_exc()), protocol=4)
        with os.fdopen(w, "wb") as f:
            f.write(data)
        os._exit(0)
    os.close(w)
    with os.fdopen(r, "rb") as f:
        data = f.read()
    os.waitpid(pid, 0)
    if not data:
        raise RuntimeError("child died without a result")
    status, payload = pickle.loads(data)
    if status == "err":
        raise RuntimeError(payload)
    return payload


# ------------------------------------------------------------------ shard

class ShardWriter:
    def __init__(self, tmp):
        self.tmp = tmp
        os.makedirs(os.path.join(tmp, "hooks"), exist_ok=True)
        self.files = {}
        self.impure, self.trace, self.patterns = set(), {}, {}

    def f(self, name):
        if name not in self.files:
            self.files[name] = open(os.path.join(self.tmp, name), "a", encoding="utf-8")
        return self.files[name]

    def write(self, name, obj):
        self.f(name).write(dump_line(obj) + "\n")

    def absorb(self, res):
        for t in res["e2e"]:
            self.write("e2e.jsonl", t)
        for m, h, rec in res["hooks"]:
            self.write(f"hooks/{m}.{h}.jsonl", rec)
        for key in ("functions", "re", "strs", "random"):
            for r in res.get(key) or []:
                self.write(f"{key}.jsonl", r)
        for fl in res.get("floats") or []:
            self.write("floats.jsonl", canon.encode(fl))
        self.impure.update(res.get("impure", []))
        for k, (n, first) in res.get("trace", {}).items():
            e = self.trace.setdefault(k, [0, first])
            e[0] += n

    def close(self):
        for f in self.files.values():
            f.close()
        json.dump({"impure": sorted(self.impure), "trace": self.trace, "patterns": sorted(
            [p, fl] for (p, fl) in self.patterns)}, open(os.path.join(self.tmp, "meta.json"), "w"), sort_keys=True)


def run_group(convs, extras_every, writer, base_index):
    seed, clock = convs[0][1]["seed"], convs[0][1]["clock"]
    r, w = os.pipe()
    pid = os.fork()
    if pid:
        os.close(w)
        with os.fdopen(r, "rb") as f:
            payload = f.read()
        os.waitpid(pid, 0)
        if not payload:
            raise RuntimeError("group process died")
        return pickle.loads(payload)
    os.close(r)
    out = None
    try:
        probes.install_re()
        oracle.load(seed, clock)
        oracle.install_trace()
        pats = list(probes.ReLog.patterns)
        for idx, conv in convs:
            extras = idx % extras_every == 0
            t0 = time.time()
            res = fork_run(lambda: run_conversation(conv, extras))
            writer.absorb(res)
            print(f"  conv {conv['id']} ({len(conv['turns'])} turns) {time.time() - t0:.1f}s", file=sys.stderr, flush=True)
        for f in writer.files.values():
            f.close()
        out = pickle.dumps((pats, writer.impure, writer.trace), protocol=4)
    except BaseException:  # noqa: BLE001
        import traceback
        traceback.print_exc()
        out = b""
    with os.fdopen(w, "wb") as f:
        f.write(out)
    os._exit(0 if out else 1)


def cmd_shard(a):
    i, n = map(int, a.shard.split("/"))
    final = os.path.join(a.out, "shards", f"{i}of{n}")
    if os.path.exists(os.path.join(final, "DONE")):
        print(f"shard {i}/{n} already done", file=sys.stderr)
        return 0
    convs = list(enumerate(select(a.corpus, a.sample)))
    mine = block(convs, i, n)
    tmp = final + ".tmp"
    shutil.rmtree(tmp, ignore_errors=True)
    writer = ShardWriter(tmp)
    groups = {}
    for idx, c in mine:
        groups.setdefault((c["seed"], c["clock"]), []).append((idx, c))
    for key in sorted(groups):
        pats, impure, trace = run_group(groups[key], a.extras_every, writer, 0)
        writer.impure |= impure
        for p in pats:
            writer.patterns[p] = True
        for k, (cnt, first) in trace.items():
            e = writer.trace.setdefault(k, [0, first])
            e[0] += cnt
    writer.close()
    shutil.rmtree(final, ignore_errors=True)
    os.rename(tmp, final)
    open(os.path.join(final, "DONE"), "w").write(f"{len(mine)} conversations\n")
    print(f"shard {i}/{n}: {len(mine)} conversations", file=sys.stderr)
    return 0


# ------------------------------------------------------------------ merge

def cat(shards, rel, dest, gz=False):
    n = 0
    with open_text(dest + (".gz" if gz else ""), "wt") as out:
        for sd in shards:
            p = os.path.join(sd, rel)
            if os.path.exists(p):
                with open(p, encoding="utf-8") as f:
                    for line in f:
                        out.write(line)
                        n += 1
    return n


def cmd_merge(a):
    n = a.shards
    dirs = [os.path.join(a.out, "shards", f"{i}of{n}") for i in range(n)]
    for d in dirs:
        if not os.path.exists(os.path.join(d, "DONE")):
            print(f"shard {d} is not finished", file=sys.stderr)
            return 1
    name = os.path.splitext(os.path.basename(a.corpus))[0]
    for sub in ("e2e", "hooks", "functions", "pyrt", "trace", "perf"):
        shutil.rmtree(os.path.join(a.out, sub), ignore_errors=True) if sub != "perf" else None
        os.makedirs(os.path.join(a.out, sub), exist_ok=True)
    stats = {}
    stats["turns"] = cat(dirs, "e2e.jsonl", os.path.join(a.out, "e2e", name + ".jsonl"))
    hook_files = sorted({f for d in dirs for f in os.listdir(os.path.join(d, "hooks"))})
    stats["hook_calls"] = 0
    for hf in hook_files:
        stats["hook_calls"] += cat(dirs, os.path.join("hooks", hf), os.path.join(a.out, "hooks", hf), gz=True)
    meta = [json.load(open(os.path.join(d, "meta.json"))) for d in dirs]
    impure = set().union(*[set(m["impure"]) for m in meta])

    # functions
    per, sigs = {}, set()
    for d in dirs:
        p = os.path.join(d, "functions.jsonl")
        if os.path.exists(p):
            for line in open(p, encoding="utf-8"):
                r = json.loads(line)
                if r["fn"] in impure:
                    continue
                key = (r["fn"], json.dumps(r["args"]), r["clock"], r["seed"])
                if key in sigs or len(per.get(r["fn"], [])) >= FUNC_CAP:
                    continue
                sigs.add(key)
                per.setdefault(r["fn"], []).append(r)
    for fn, recs in per.items():
        with open(os.path.join(a.out, "functions", fn + ".jsonl"), "w", encoding="utf-8") as f:
            for r in recs:
                f.write(dump_line(r) + "\n")
    stats["functions"] = len(per)
    stats["function_calls"] = sum(len(v) for v in per.values())

    # pyrt
    def read_all(rel):
        for d in dirs:
            p = os.path.join(d, rel)
            if os.path.exists(p):
                for line in open(p, encoding="utf-8"):
                    yield json.loads(line)

    def emit(kind, cases):
        count = 0
        with open(os.path.join(a.out, "pyrt", kind + ".jsonl"), "w", encoding="utf-8") as f:
            for case in cases:
                exp = pyrt_cases.run_case(kind, case)
                f.write(dump_line({"id": count, "case": case, "expect": exp}) + "\n")
                count += 1
        stats["pyrt_" + kind] = count

    seen, res, pat_n = {}, [], {}
    for c in read_all("re.jsonl"):
        key = (c["pattern"], c["flags"], c["method"])
        sig = json.dumps(c["args"])
        if seen.setdefault(key, set()).__len__() >= RE_CAP or sig in seen[key]:
            continue
        pk = (c["pattern"], c["flags"])
        if pat_n.get(pk, 0) >= RE_PAT_CAP:
            continue
        pat_n[pk] = pat_n.get(pk, 0) + 1
        seen[key].add(sig)
        res.append(c)
    emit("re", res)
    strs, sseen = [], set()
    for c in read_all("strs.jsonl"):
        sig = json.dumps(c)
        if sig not in sseen and len(strs) < 20000:
            sseen.add(sig)
            strs.append(c)
    emit("str", strs + pyrt_cases.synth_str())
    fmt = pyrt_cases.synth_format()
    for fl in read_all("floats.jsonl"):
        fmt.append({"op": "repr", "value": fl})
        fmt.append({"op": "str", "value": fl})
    emit("format", fmt)
    rnd = list(pyrt_cases.synth_random())
    rseen = set()
    for c in read_all("random.jsonl"):
        sig = json.dumps(c, sort_keys=True)
        if sig not in rseen and len(rnd) < 6000:
            rseen.add(sig)
            rnd.append({"seed": c["seed"], "op": c["op"], "args": c["args"]})
    emit("random", rnd)

    # trace
    trace = {}
    for m in meta:
        for k, (cnt, first) in m["trace"].items():
            e = trace.setdefault(k, [0, first])
            e[0] += cnt
    json.dump({k: {"count": v[0], "first": v[1]} for k, v in sorted(trace.items())},
              open(os.path.join(a.out, "trace", "calls.json"), "w"), indent=0)
    pats = sorted({tuple(p) for m in meta for p in m["patterns"]})
    json.dump([list(p) for p in pats], open(os.path.join(a.out, "trace", "re_patterns.json"), "w"))
    stats["trace_keys"], stats["re_patterns"] = len(trace), len(pats)

    manifest = {"corpus": name, "corpus_sha256": __import__("hashlib").sha256(open(a.corpus, "rb").read()).hexdigest(),
                "sample_percent": a.sample, "shards": n, "python": sys.version.split()[0], "pybot_commit": paths.PYBOT_COMMIT,
                "stats": stats, "turn_index_base": 0}
    json.dump(manifest, open(os.path.join(a.out, "MANIFEST.json"), "w"), indent=1, sort_keys=True)
    print(json.dumps(stats), file=sys.stderr)
    return 0


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("shard", "merge"):
        p = sub.add_parser(name)
        p.add_argument("--corpus", default=paths.corpus_path())
        p.add_argument("--out", default=paths.goldens_dir())
        p.add_argument("--sample", type=int, default=100)
        if name == "shard":
            p.add_argument("--shard", required=True)
            p.add_argument("--extras-every", type=int, default=3)
        else:
            p.add_argument("--shards", type=int, required=True)
    a = ap.parse_args()
    sys.exit(cmd_shard(a) if a.cmd == "shard" else cmd_merge(a))


if __name__ == "__main__":
    main()

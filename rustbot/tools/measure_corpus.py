#!/usr/bin/env python3
"""Measure corpus coverage with one fresh process per seed/clock group.

This is an alternative runner, not a repair or successful execution of the
protected build_corpus.py measure command. Oracle/probe sources are read-only.
Each group loads PyBot once; each conversation runs in a forked child of that
pristine loaded parent. A report is replaced atomically only after all inputs
succeed and their conversation/turn counts have been checked.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor, as_completed
import hashlib
import json
import multiprocessing
import os
from pathlib import Path
import pickle
import selectors
import signal
import sys
import tempfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[2]
CHECKS = ROOT / "agent" / "checks"
sys.path.insert(0, str(CHECKS))
DEFAULT_CORPUS = CHECKS / "corpus" / "dev.jsonl"


def log(event, **fields):
    print(json.dumps({"event": event, **fields}, sort_keys=True), flush=True)


def fork_result(function, argument, timeout):
    """Drain before waitpid, with deadline and cleanup even for large results."""
    reader, writer = os.pipe()
    try:
        pid = os.fork()
    except BaseException:
        os.close(reader)
        os.close(writer)
        raise
    if pid == 0:
        os.close(reader)
        try:
            payload = ("ok", function(argument))
        except BaseException:
            payload = ("error", traceback.format_exc())
        try:
            with os.fdopen(writer, "wb") as output:
                pickle.dump(payload, output, protocol=4)
        finally:
            os._exit(0)
    os.close(writer)
    waited = False
    selector = selectors.DefaultSelector()
    try:
        os.set_blocking(reader, False)
        selector.register(reader, selectors.EVENT_READ)
        deadline = time.monotonic() + timeout
        data = bytearray()
        eof = False
        while not eof:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"conversation child {pid} exceeded {timeout:g} seconds")
            for _, _ in selector.select(min(remaining, 1.0)):
                while True:
                    try:
                        chunk = os.read(reader, 65536)
                    except BlockingIOError:
                        break
                    if not chunk:
                        eof = True
                        break
                    data.extend(chunk)
                    if len(data) > 256 * 1024 * 1024:
                        raise RuntimeError("conversation coverage payload exceeded 256 MiB")
        _, status = os.waitpid(pid, 0)
        waited = True
        if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
            raise RuntimeError(f"conversation child {pid} failed with wait status {status}")
        if not data:
            raise RuntimeError(f"conversation child {pid} returned no result")
        outcome, payload = pickle.loads(data)
        if outcome != "ok":
            raise RuntimeError(payload)
        return payload
    finally:
        selector.close()
        os.close(reader)
        if not waited:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.waitpid(pid, 0)


def pack_coverage(coverage):
    files = defaultdict(set)
    for path, line in coverage:
        files[path].add(line)
    return {path: sorted(lines) for path, lines in sorted(files.items())}


def measure_group(task):
    # Spawn + max_tasks_per_child=1 guarantees monitoring tool 1 is acquired
    # only once in this interpreter. Nothing resets/patches sys.monitoring.
    from lib import oracle, probes

    group_id, seed, clock, items, child_timeout = task
    if os.environ.get("PYTHONHASHSEED") != "0" or os.environ.get("_ORACLE_ENV") != "1":
        raise RuntimeError("oracle environment was not established before spawning workers")

    def terminate(signum, _frame):
        raise SystemExit(128 + signum)

    signal.signal(signal.SIGTERM, terminate)
    started = time.monotonic()
    load_seconds = oracle.load(seed, clock, probes.install_coverage)
    baseline = set(probes.Cov.hit)
    coverage = set(baseline)
    proposals, wins = Counter(), Counter()
    completed, turns = [], 0
    log("loaded", group=group_id, pid=os.getpid(), seed=seed, clock=clock,
        conversations=len(items), import_lines=len(baseline), load_seconds=round(load_seconds, 3))

    def conversation(item):
        index, conv = item
        memory = {}
        proposed, won = Counter(), Counter()
        for turn_index, text in enumerate(conv["turns"]):
            try:
                result = oracle.run_turn(text, memory)
                winner = result["module"]
                if winner:
                    won[winner] += 1
                for candidate in result["candidates"]:
                    proposed[candidate["module"]] += 1
            except BaseException as error:
                raise RuntimeError(
                    f"conversation {conv['id']!r}, turn {turn_index}, seed={seed}, clock={clock}"
                ) from error
        return {"index": index, "id": conv["id"], "turns": len(conv["turns"]),
                "proposals": dict(proposed), "wins": dict(won),
                "cover": pack_coverage(probes.Cov.hit - baseline)}

    for position, item in enumerate(items, 1):
        try:
            result = fork_result(conversation, item, child_timeout)
        except BaseException as error:
            raise RuntimeError(f"group {group_id}, conversation {item[1]['id']!r} failed") from error
        if result["index"] != item[0] or result["id"] != item[1]["id"]:
            raise RuntimeError("conversation result identity mismatch")
        if result["turns"] != len(item[1]["turns"]):
            raise RuntimeError("conversation result turn count mismatch")
        for path, lines in result["cover"].items():
            coverage.update((path, line) for line in lines)
        proposals.update(result["proposals"])
        wins.update(result["wins"])
        completed.append(result["index"])
        turns += result["turns"]
        if position % 10 == 0 or position == len(items):
            log("progress", group=group_id, pid=os.getpid(), completed=position,
                conversations=len(items), turns=turns,
                elapsed_seconds=round(time.monotonic() - started, 1))
    return {"group": group_id, "pid": os.getpid(), "seed": seed, "clock": clock,
            "indices": completed, "turns": turns, "proposals": dict(proposals),
            "wins": dict(wins), "cover": pack_coverage(coverage),
            "import_lines": len(baseline), "load_seconds": load_seconds,
            "elapsed_seconds": time.monotonic() - started}


def load_corpus(path):
    raw = path.read_bytes()
    conversations, ids = [], set()
    for line_no, line in enumerate(raw.decode("utf-8").splitlines(), 1):
        if not line.strip():
            continue
        conv = json.loads(line)
        if not isinstance(conv, dict) or not isinstance(conv.get("id"), str):
            raise ValueError(f"line {line_no}: conversation needs a string id")
        if conv["id"] in ids:
            raise ValueError(f"line {line_no}: duplicate conversation id {conv['id']!r}")
        ids.add(conv["id"])
        if type(conv.get("seed")) is not int or not isinstance(conv.get("clock"), str):
            raise ValueError(f"line {line_no}: explicit integer seed and string clock required")
        if not isinstance(conv.get("turns"), list) or not all(isinstance(t, str) for t in conv["turns"]):
            raise ValueError(f"line {line_no}: turns must be a list of strings")
        conversations.append(conv)
    if not conversations:
        raise ValueError("corpus contains no conversations")
    return conversations, hashlib.sha256(raw).hexdigest()


def coverage_report(corpus, conversations, covered, proposals, wins):
    # Mirror cmd_measure's executable-line/handler/schema calculations exactly.
    import re
    from lib import probes

    executable = probes.executable_lines()
    total = sum(map(len, executable.values()))
    hit = sum(len(lines & covered.get(path, set())) for path, lines in executable.items())
    by_package = {}
    for path, lines in executable.items():
        relative = os.path.relpath(path, probes.BOT_DIR)
        package = relative.split(os.sep)[0] if os.sep in relative else "."
        previous_total, previous_hit = by_package.get(package, (0, 0))
        by_package[package] = (previous_total + len(lines), previous_hit + len(lines & covered.get(path, set())))
    handlers = []
    for file in sorted((Path(probes.BOT_DIR) / "modules").glob("*.py")):
        if file.name != "__init__.py" and re.search(r"^def respond\(|^respond\s*=|^from .* import .*\brespond\b", file.read_text(encoding="utf-8"), re.M):
            handlers.append(file.stem)
    handlers = sorted(set(handlers) | set(proposals))
    return {"corpus": corpus.name, "conversations": len(conversations),
            "turns": sum(len(c["turns"]) for c in conversations),
            "lines_executable": total, "lines_covered": hit,
            "line_coverage_pct": round(100.0 * hit / total, 2),
            "by_package": {name: {"executable": count, "covered": covered_count,
                                    "pct": round(100.0 * covered_count / count, 1)}
                           for name, (count, covered_count) in sorted(by_package.items())},
            "handlers": len(handlers), "proposed": len(proposals), "won": len(wins),
            "never_proposed": sorted(set(handlers) - set(proposals)),
            "never_won": sorted(set(handlers) - set(wins)),
            "proposals": dict(sorted(proposals.items())), "wins": dict(sorted(wins.items()))}


def atomic_json(path, report):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=path.parent,
                                         prefix=".coverage-isolated-", suffix=".tmp", delete=False) as output:
            temporary = Path(output.name)
            json.dump(report, output, indent=1)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main():
    from lib import oracle
    os.environ.setdefault("PYTHONPYCACHEPREFIX", str(ROOT / "rustbot" / "target" / "pycache-isolated"))
    oracle.ensure_env()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--jobs", type=int, default=6)
    parser.add_argument("--child-timeout", type=float, default=1800,
                        help="maximum seconds per conversation; any timeout aborts the report")
    args = parser.parse_args()
    if not 1 <= args.jobs <= 6 or args.child_timeout <= 0:
        parser.error("jobs must be in 1..6 and child timeout must be positive")
    corpus = args.corpus.resolve()
    out = (args.out or corpus.with_suffix(".coverage.json")).resolve()
    conversations, corpus_hash = load_corpus(corpus)
    groups = defaultdict(list)
    for index, conv in enumerate(conversations):
        groups[(conv["seed"], conv["clock"])].append((index, conv))
    ordered = sorted(groups.items())
    tasks = [(group_id, seed, clock, items, args.child_timeout)
             for group_id, ((seed, clock), items) in enumerate(ordered)]
    tasks.sort(key=lambda task: (-len(task[3]), task[0]))
    expected_turns = sum(len(c["turns"]) for c in conversations)
    log("start", corpus=str(corpus), corpus_sha256=corpus_hash, conversations=len(conversations),
        turns=expected_turns, groups=len(tasks), jobs=args.jobs, output=str(out),
        process_model="spawned group worker; max_tasks_per_child=1; fork per conversation")
    started = time.monotonic()
    covered, proposals, wins, completed = defaultdict(set), Counter(), Counter(), set()
    turns = 0
    metadata = []
    context = multiprocessing.get_context("spawn")
    executor = ProcessPoolExecutor(max_workers=args.jobs, mp_context=context, max_tasks_per_child=1)
    try:
        futures = {executor.submit(measure_group, task): task[0] for task in tasks}
        for future in as_completed(futures):
            group_id = futures.pop(future)
            result = future.result()
            if result["group"] != group_id or completed.intersection(result["indices"]):
                raise RuntimeError("duplicate or mismatched group results")
            completed.update(result["indices"])
            turns += result["turns"]
            proposals.update(result["proposals"])
            wins.update(result["wins"])
            for path, lines in result["cover"].items():
                covered[path].update(lines)
            metadata.append({key: result[key] for key in ("group", "pid", "seed", "clock", "turns", "import_lines", "load_seconds", "elapsed_seconds")})
            metadata[-1]["conversations"] = len(result["indices"])
            log("group_done", group=group_id, completed_conversations=len(completed),
                conversations=len(conversations), turns=turns, completed_groups=len(metadata), groups=len(tasks))
    except BaseException:
        executor.terminate_workers()
        raise
    else:
        executor.shutdown(wait=True)
    if completed != set(range(len(conversations))) or turns != expected_turns:
        raise RuntimeError(f"incomplete measurement: {len(completed)} conversations/{turns} turns")
    if len({group["pid"] for group in metadata}) != len(tasks):
        raise RuntimeError("a group process was unexpectedly reused")
    report = coverage_report(corpus, conversations, covered, proposals, wins)
    report["measurement"] = {"tool": "rustbot/tools/measure_corpus.py", "corpus_sha256": corpus_hash,
                             "method": "isolated seed/clock groups; pristine imported parent forked per conversation",
                             "note": "Alternative measurement; protected build_corpus.py measure is unchanged and is not claimed to pass.",
                             "monitoring_installs_per_worker": 1, "jobs": args.jobs,
                             "groups": sorted(metadata, key=lambda group: group["group"]),
                             "elapsed_seconds": round(time.monotonic() - started, 3)}
    atomic_json(out, report)
    log("complete", conversations=report["conversations"], turns=report["turns"],
        coverage_pct=report["line_coverage_pct"], proposed=report["proposed"],
        handlers=report["handlers"], won=report["won"], output=str(out))


if __name__ == "__main__":
    main()

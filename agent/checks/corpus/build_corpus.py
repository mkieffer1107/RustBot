#!/usr/bin/env python3
"""Builds the conversation corpus (default corpus/dev.jsonl) deterministically.

    build_corpus.py [build] [--seed N] [--out PATH]      assemble the corpus (cheap; needs pyarrow or `uv`)
    build_corpus.py mine [--jobs J] [--pool N]           run candidate phrases mined from bot/modules through PyBot and
                                                         keep those that add coverage -> corpus/mined.json (slow, once)
    build_corpus.py measure [--corpus C] [--jobs J]      run the corpus through PyBot: line coverage and which modules
                                                         propose / win -> corpus/<name>.coverage.json

One conversation per line: {"id", "seed", "clock", "turns": [...]}. The conversation seed/clock are the pybot_web SEED
and the frozen clock. The builder --seed only changes how conversations are drawn (the hold-out uses another one).
"""
import argparse
import ast
import json
import os
import pickle
import random
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
CHECKS = os.path.dirname(HERE)
sys.path.insert(0, CHECKS)

CLOCKS = ["2026-06-15T12:00:00",   # Monday
          "2026-06-13T09:30:00",   # Saturday
          "2028-02-29T23:15:00",   # leap day
          "2026-12-31T18:45:00",   # new year's eve
          "2027-03-28T01:30:00"]   # a Sunday around the DST change
SEEDS = [0, 0, 0, 0, 1, 2, 3, 7, 42]
PARTS = 16

# ---------------------------------------------------------------------------------------------------------------
# prompts

def smoltalk():
    path = os.path.join(os.path.dirname(os.path.dirname(CHECKS)), "reference", "smoltalkbench", "test.parquet")
    try:
        import pyarrow.parquet as pq
    except ImportError:
        if os.environ.get("_CORPUS_UV") == "1":
            raise
        env = dict(os.environ, _CORPUS_UV="1")
        os.execvpe("uv", ["uv", "run", "--quiet", "--with", "pyarrow", "python", os.path.abspath(__file__)] + sys.argv[1:], env)
    return pq.read_table(path).column("instruction").to_pylist()


NAMES = ["Anna", "Tom", "Priya", "Mohammed", "Sofia", "Liam", "Chen", "Aisha", "Jack", "Maria", "Olu", "Hannah", "Raj",
         "Ellie", "Dmitri", "Zoe", "Ben", "Fatima", "Noah", "Ingrid"]
CITIES = ["London", "Leeds", "Cardiff", "Glasgow", "Paris", "Berlin", "Lagos", "Mumbai", "Toronto", "Sydney", "Boston",
          "Madrid", "Dublin", "Manchester", "Oslo"]
JOBS = ["nurse", "teacher", "plumber", "software engineer", "chef", "bus driver", "student", "carpenter", "accountant",
        "farmer", "pharmacist", "electrician"]
PETS = [("dog", "Rex"), ("cat", "Luna"), ("rabbit", "Biscuit"), ("parrot", "Kiwi"), ("hamster", "Peanut")]
HOBBIES = ["gardening", "football", "knitting", "chess", "running", "baking", "painting", "fishing", "cycling", "hiking",
           "reading", "photography", "climbing", "singing"]
RELATIVES = ["mum", "dad", "grandad", "gran", "brother", "sister", "uncle", "aunt", "husband", "wife", "friend"]
FOODS = ["lasagne", "curry", "pancakes", "sushi", "soup", "pizza", "porridge", "tacos"]

FOLLOWUPS = ["how long does that take", "can you say more", "why", "what do you mean", "thanks", "ok", "and then?",
             "what about the cost", "tell me more", "is that safe", "can you give me an example", "who else knows",
             "what should i do first", "really?", "and if it goes wrong", "how much is that", "where do i start",
             "what was that again", "can you repeat that", "no", "yes please", "i don't understand", "that's not what i asked",
             "what's my name?", "do you remember what i said?", "what did i tell you earlier", "never mind", "lol",
             "good morning", "bye", "hello again", "are you a robot", "what can you do", "who made you"]

STATE_LINES = ["i feel really down today", "i'm so tired of everything", "i'm worried about my exam", "i just got the job!",
               "my {rel} passed away last month", "i had a row with my {rel}", "i'm so angry right now",
               "i can't sleep", "i'm scared of flying", "i'm lonely", "my dog died yesterday", "i failed my driving test",
               "i got engaged", "we're expecting a baby", "i lost my wallet", "my boss shouted at me"]

URGENT = ["i can't breathe", "my chest hurts really badly", "i think i'm having a stroke", "i want to hurt myself",
          "there's a fire in my kitchen", "my baby is not breathing", "i took too many tablets", "i feel like i want to die"]

UTILITY = ["what day is it", "what's the date today", "how many days until christmas", "is it the weekend", "what time is it",
           "what is 15% of 240", "convert 5 miles to km", "what's 12 times 13", "how many days in february",
           "what's the weather like", "tell me a joke", "what year is it", "how many weeks until new year",
           "when is easter", "is it a leap year", "how old would someone born in 1990 be", "what's 7 squared",
           "how many ounces in a pound", "what is the capital of france", "when is mother's day"]


def memory_scripts(rng):
    name, name2 = rng.sample(NAMES, 2)
    city, city2 = rng.sample(CITIES, 2)
    job = rng.choice(JOBS)
    pet, pname = rng.choice(PETS)
    hobby, hobby2 = rng.sample(HOBBIES, 2)
    rel = rng.choice(RELATIVES)
    age = rng.randint(7, 88)
    food = rng.choice(FOODS)
    scripts = [
        ["hi", f"my name is {name}", "what's my name?"],
        [f"i'm {name} and i'm {age}", "how old am i?", f"i live in {city}", "where do i live?", "what do you know about me?"],
        [f"i work as a {job}", "what do i do for a living?", f"my {pet} is called {pname}", f"what's my {pet}'s name?"],
        [f"my name is {name}", f"actually it's {name2}", "what is my name"],
        [f"i love {hobby}", "what do i like?", f"tell me something about {hobby}", f"i also like {hobby2}", "what are my hobbies"],
        [f"i moved to {city} last year", f"i grew up in {city2}", "where am i from?", "how long have i lived here?"],
        [f"my {rel} is called {rng.choice(NAMES)}", f"what's my {rel}'s name?", f"my {rel} is ill", "thank you"],
        [f"i'm making {food} tonight", f"how long does {food} take", "what do i need", "any tips?", "thanks!"],
        [f"i turn {age + 1} next month", "how old will i be?", "when's my birthday?", f"it's on the {rng.randint(1, 28)}th"],
        [f"i am {age} years old", f"i was born in {2026 - age}", "how old am i?"],
        ["i've got an interview on friday", "how should i prepare?", "what if i'm late", "it went badly", "what now"],
        [f"i have a {pet}", f"{pname} is {rng.randint(1, 15)}", "how old is my pet?", f"{pname} is ill", "what should i do"],
        [rng.choice(STATE_LINES).format(rel=rel), rng.choice(FOLLOWUPS), rng.choice(STATE_LINES).format(rel=rel), "thanks"],
        [rng.choice(UTILITY), rng.choice(UTILITY), rng.choice(UTILITY), rng.choice(FOLLOWUPS)],
        ["i want to save money", "how much should i save each month", "i earn 2000 a month", "what's a good budget", "thanks"],
        [f"i'm going to {city} next week", "what should i pack", "how long is the flight", "what time is it there", "any tips?"],
        [f"i'm {name}", "i'm tired", "i'm hungry", "what's my name?", "what am i?", "who am i"],
        ["can you help me with my homework", "it's maths", "what is 144 divided by 12", "and 1.5 times 8", "thank you"],
    ]
    return rng.choice(scripts)


def typo(word, rng):
    if len(word) < 4:
        return word
    i = rng.randrange(1, len(word) - 1)
    k = rng.randrange(3)
    if k == 0:
        return word[:i] + word[i + 1] + word[i] + word[i + 2:]
    if k == 1:
        return word[:i] + word[i + 1:]
    return word[:i] + word[i] + word[i:]


CHAT = [(r"\byou\b", "u"), (r"\bare\b", "r"), (r"\bplease\b", "plz"), (r"\bbecause\b", "cuz"), (r"\bthanks\b", "thx"),
        (r"\bwhat's\b", "whats"), (r"\bI'm\b", "im"), (r"\bdon't\b", "dont"), (r"\bto\b", "2"), (r"\bfor\b", "4"),
        (r"\bwith\b", "w/"), (r"\bgoing to\b", "gonna"), (r"\bwant to\b", "wanna")]


def perturb(text, rng):
    k = rng.randrange(8)
    if k == 0:
        return text.lower()
    if k == 1:
        return text.upper()
    if k == 2:
        return re.sub(r"[?.!,;:]", "", text)
    if k == 3:
        return "  " + text.replace(" ", "  ", 2) + "   "
    if k == 4:
        words = text.split(" ")
        for _ in range(2):
            i = rng.randrange(len(words))
            words[i] = typo(words[i], rng)
        return " ".join(words)
    if k == 5:
        t = text.lower()
        for pat, rep in CHAT:
            t = re.sub(pat, rep, t, flags=re.I)
        return t
    if k == 6:
        return text.rstrip("?.!") + rng.choice(["???", "!!", "...", " :)", " lol", " 😀"])
    return rng.choice(["hey, ", "so ", "um ", "ok so ", "quick question: "]) + text[:1].lower() + text[1:]


# ---------------------------------------------------------------------------------------------------------------
# mining phrases out of bot/modules/*.py

def regex_sampler():
    import re._parser as sp  # stdlib parser (internal module, used only to *generate* test input)
    from re import _constants as C

    def sample(pattern, rng):
        tree = sp.parse(pattern)
        groups = {}

        def cat(code):
            n = str(code)
            if "DIGIT" in n:
                return "4" if "NOT" not in n else "x"
            if "SPACE" in n:
                return " " if "NOT" not in n else "x"
            if "WORD" in n:
                return "a" if "NOT" not in n else " "
            return "a"

        def one_in(items):
            neg = any(op == C.NEGATE for op, _ in items)
            chars = []
            for op, av in items:
                if op == C.LITERAL:
                    chars.append(chr(av))
                elif op == C.RANGE:
                    chars.append(chr(rng.randint(av[0], min(av[1], av[0] + 25))))
                elif op == C.CATEGORY:
                    chars.append(cat(av))
            if neg:
                return next((c for c in "zq9" if c not in chars), "z")
            return rng.choice(chars) if chars else "a"

        def walk(seq):
            out = []
            for op, av in seq:
                if op == C.LITERAL:
                    out.append(chr(av))
                elif op == C.NOT_LITERAL:
                    out.append("z" if chr(av) != "z" else "q")
                elif op == C.ANY:
                    out.append("a")
                elif op == C.IN:
                    out.append(one_in(av))
                elif op == C.CATEGORY:
                    out.append(cat(av))
                elif op == C.BRANCH:
                    out.append(walk(rng.choice(av[1])))
                elif op == C.SUBPATTERN:
                    g = av[0]
                    s = walk(av[3])
                    if g is not None:
                        groups[g] = s
                    out.append(s)
                elif op in (C.MAX_REPEAT, C.MIN_REPEAT) or getattr(C, "POSSESSIVE_REPEAT", None) == op:
                    lo, hi, sub = av
                    hi = min(hi, lo + 2) if hi != C.MAXREPEAT else lo + 2
                    n = lo if rng.random() < 0.6 else rng.randint(lo, max(lo, hi))
                    out.append("".join(walk(sub) for _ in range(n)))
                elif getattr(C, "ATOMIC_GROUP", None) == op:
                    out.append(walk(av))
                elif op == C.GROUPREF:
                    out.append(groups.get(av, ""))
                elif op == C.GROUPREF_EXISTS:
                    out.append(walk(av[1]))
                # AT, ASSERT, ASSERT_NOT: nothing to emit
            return "".join(out)
        return walk(tree)
    return sample


def mine_pool(rng, per_file=70):
    """Candidate user lines from every bot/modules/*.py: samples of its regexes plus its short literal phrases."""
    sample = regex_sampler()
    bot = os.path.join(os.path.dirname(os.path.dirname(CHECKS)), "reference", "pybot", "bot")
    pool = {}
    work = [("modules", fn, per_file) for fn in sorted(os.listdir(os.path.join(bot, "modules")))]
    for sub in ("understanding", "memory", "compose", "knowledge"):
        work += [(sub, fn, max(8, per_file // 5)) for fn in sorted(os.listdir(os.path.join(bot, sub)))]
    for sub, fn, per_file in work:
        if not fn.endswith(".py") or fn == "__init__.py":
            continue
        try:
            tree = ast.parse(open(os.path.join(bot, sub, fn), encoding="utf-8").read())
        except SyntaxError:
            continue
        pats, phrases = [], []
        for node in ast.walk(tree):
            if isinstance(node, ast.Constant) and isinstance(node.value, str):
                v = node.value
                if 4 <= len(v) <= 1500 and "\n" not in v and re.search(r"[\\(\[|?*+]", v) and " " in v + " ":
                    pats.append(v)
                elif 6 <= len(v) <= 80 and re.fullmatch(r"[a-z][a-z' ,?.-]+", v) and v.count(" ") >= 1:
                    phrases.append(v)
        cands = []
        for p in sorted(set(pats)):
            try:
                re.compile(p)
            except re.error:
                continue
            for _ in range(2):
                try:
                    s = sample(p, rng).strip()
                except Exception:  # noqa: BLE001
                    continue
                if 3 <= len(s) <= 140 and re.search(r"[a-z]{2}", s, re.I):
                    try:
                        if re.search(p, s):
                            cands.append(s)
                    except re.error:
                        pass
        cands += sorted(set(phrases))
        cands = sorted(set(cands))
        rng.shuffle(cands)
        pool[f"{sub}/{fn[:-3]}" if sub != "modules" else fn[:-3]] = cands[:per_file]
    return pool


# ---------------------------------------------------------------------------------------------------------------
# evaluation workers (run PyBot, collect coverage and proposers)

def eval_part(convs_path, part, out_path, mode):
    """Run one logical part of a conversation list. mode: 'mine' (select) or 'measure' (just run)."""
    from lib import oracle
    oracle.ensure_env()
    from lib import probes
    convs = [json.loads(l) for l in open(convs_path, encoding="utf-8") if l.strip()]
    mine = [(i, c) for i, c in enumerate(convs) if i % PARTS == part]
    files = {}

    def lid(path, line):
        return files.setdefault(path, len(files)) << 20 | line

    results, cover = [], set()
    groups = {}
    for i, c in mine:
        groups.setdefault((c["seed"], c["clock"]), []).append((i, c))
    seen_prop, seen_win = set(), set()
    for (seed, clock), items in sorted(groups.items()):
        probes.install_coverage()
        oracle.load(seed, clock)
        base = {lid(*k) for k in probes.Cov.hit}
        cover |= base
        for i, c in items:
            r, w = os.pipe()
            pid = os.fork()
            if pid == 0:
                os.close(r)
                try:
                    mem, turns = {}, []
                    for t in c["turns"]:
                        res = oracle.run_turn(t, mem)
                        turns.append([res["module"], [x["module"] for x in res["candidates"]]])
                    lines = sorted({lid(*k) for k in probes.Cov.hit} - base)
                    data = pickle.dumps((turns, lines))
                except BaseException:  # noqa: BLE001
                    data = b""
                with os.fdopen(w, "wb") as f:
                    f.write(data)
                os._exit(0)
            os.close(w)
            data = open(r, "rb").read()
            os.waitpid(pid, 0)
            if not data:
                results.append({"i": i, "error": True})
                continue
            turns, lines = pickle.loads(data)
            new = [l for l in lines if l not in cover]
            props = {m for _, ps in turns for m in ps}
            wins = {w for w, _ in turns if w}
            fresh = (props - seen_prop) or (wins - seen_win)
            accept = len(new) >= 12 or bool(fresh)
            cover.update(lines)
            seen_prop |= props
            seen_win |= wins
            results.append({"i": i, "turns": turns, "new_lines": len(new), "accept": accept})
    inv = {v: k for k, v in files.items()}
    with open(out_path, "wb") as f:
        pickle.dump({"results": results, "cover": sorted(cover), "files": inv}, f)


def run_parts(convs_path, mode, jobs, workdir):
    procs, outs = [], []
    os.makedirs(workdir, exist_ok=True)
    pending = list(range(PARTS))
    running = []
    while pending or running:
        while pending and len(running) < jobs:
            p = pending.pop(0)
            out = os.path.join(workdir, f"part{p}.pkl")
            outs.append(out)
            cmd = [sys.executable, os.path.abspath(__file__), "_eval", "--convs", convs_path, "--part", str(p), "--out", out,
                   "--mode", mode]
            running.append((p, subprocess.Popen(cmd, stdout=subprocess.DEVNULL)))
        for p, pr in list(running):
            if pr.poll() is not None:
                running.remove((p, pr))
                if pr.returncode != 0:
                    raise SystemExit(f"eval part {p} failed ({pr.returncode})")
                print(f"  part {p} done", file=sys.stderr, flush=True)
        time.sleep(0.5)
    return [pickle.load(open(o, "rb")) for o in sorted(outs, key=lambda s: int(re.findall(r"part(\d+)", s)[0]))]


def executable_total():
    from lib import probes
    return probes.executable_lines()


def cmd_mine(a):
    rng = random.Random(1)
    pool = mine_pool(rng, a.pool)
    flat = [(m, s) for m in sorted(pool) for s in pool[m]]
    work = os.path.join(a.work, "mine")
    os.makedirs(work, exist_ok=True)
    convs_path = os.path.join(work, "pool.jsonl")
    order = list(range(len(flat)))
    rng.shuffle(order)
    with open(convs_path, "w", encoding="utf-8") as f:
        for n, idx in enumerate(order):
            f.write(json.dumps({"id": f"m{n}", "seed": 0, "clock": CLOCKS[0], "turns": [flat[idx][1]]}) + "\n")
    print(f"mining: {len(flat)} candidate lines from {len(pool)} modules", file=sys.stderr)
    parts = run_parts(convs_path, "mine", a.jobs, work)
    chosen = []
    for part in parts:
        for r in part["results"]:
            if r.get("accept"):
                m, s = flat[order[r["i"]]]
                chosen.append({"module": m, "text": s, "winner": r["turns"][0][0], "proposers": r["turns"][0][1]})
    chosen.sort(key=lambda c: (c["module"], c["text"]))
    json.dump({"note": "phrases mined from reference/pybot/bot/modules that added PyBot coverage; see build_corpus.py mine",
               "candidates": len(flat), "phrases": chosen}, open(os.path.join(HERE, "mined.json"), "w"), indent=0,
              ensure_ascii=False)
    print(f"kept {len(chosen)} of {len(flat)}", file=sys.stderr)


def split_prompts(prompts, split):
    """Disjoint SmolTalkBench prompt sets: 'dev' (~70%) for the agent, 'holdout' (~30%) for the grader only.
    Assignment is by a stable hash of the prompt text, so it never depends on order or seed."""
    if split == "all":
        return prompts
    import hashlib
    held = lambda t: int(hashlib.sha256(t.encode("utf-8")).hexdigest(), 16) % 10 < 3
    return [t for t in prompts if held(t) == (split == "holdout")]


# ---------------------------------------------------------------------------------------------------------------
# assembling the corpus

def build(a):
    rng = random.Random(f"corpus:{a.seed}")
    prompts = split_prompts(smoltalk(), a.split)
    mined_path = os.path.join(HERE, "mined.json")
    if os.path.exists(mined_path):
        mined = json.load(open(mined_path, encoding="utf-8"))["phrases"]
    else:
        print("corpus/mined.json missing: using unfiltered regex samples (run `build_corpus.py mine`)", file=sys.stderr)
        pool = mine_pool(random.Random(1), 25)
        mined = [{"module": m, "text": s} for m in sorted(pool) for s in pool[m]]
    mined_text = [m["text"] for m in mined]
    convs = []

    def add(kind, turns):
        convs.append({"kind": kind, "turns": [t for t in turns if t.strip()] or ["hello"]})

    order = list(range(len(prompts)))
    rng.shuffle(order)
    for i in order:                                    # every gold prompt once, alone
        add("st", [prompts[i]])
    for _ in range(260):                               # gold prompts with follow-ups
        n = rng.choice([2, 2, 3, 3, 4])
        turns = []
        for _ in range(n):
            turns.append(rng.choice(prompts) if rng.random() < 0.55 else rng.choice(FOLLOWUPS))
        turns[0] = rng.choice(prompts)
        add("stf", turns)
    for _ in range(300):                               # perturbed gold prompts
        t = [perturb(rng.choice(prompts), rng)]
        if rng.random() < 0.4:
            t.append(perturb(rng.choice(FOLLOWUPS), rng))
        add("pert", t)
    for _ in range(420):                               # memory scripts
        s = memory_scripts(rng)
        if rng.random() < 0.3:
            s = [perturb(x, rng) if rng.random() < 0.4 else x for x in s]
        add("mem", s)
    for _ in range(320):                               # mined module phrases, alone or in context
        m = rng.choice(mined_text)
        kind = rng.randrange(3)
        if kind == 0:
            t = [m]
        elif kind == 1:
            t = [rng.choice(prompts), m]
        else:
            t = [memory_scripts(rng)[0], m, rng.choice(mined_text)]
        add("mine", t)
    for _ in range(80):
        add("urg", [rng.choice(URGENT)] + ([rng.choice(FOLLOWUPS)] if rng.random() < .6 else []))
    for _ in range(60):
        add("util", [rng.choice(UTILITY) for _ in range(rng.randint(1, 3))])
    for _ in range(260):                               # long mixed conversations
        pieces = []
        for _ in range(rng.randint(6, 8)):
            r = rng.random()
            if r < .25:
                pieces.append(rng.choice(prompts))
            elif r < .5:
                pieces.append(rng.choice(mined_text))
            elif r < .65:
                pieces.append(rng.choice(FOLLOWUPS))
            elif r < .8:
                pieces.append(rng.choice(STATE_LINES).format(rel=rng.choice(RELATIVES)))
            elif r < .9:
                pieces.append(rng.choice(UTILITY))
            else:
                pieces.extend(memory_scripts(rng)[:2])
        add("long", pieces[:8])
    rng.shuffle(convs)
    pfx = f"d{a.seed}"
    with open(a.out, "w", encoding="utf-8") as f:
        for n, c in enumerate(convs):
            seed = rng.choice(SEEDS)
            clock = CLOCKS[0] if rng.random() < 0.5 else rng.choice(CLOCKS)
            f.write(json.dumps({"id": f"{pfx}-{c['kind']}-{n:05d}", "seed": seed, "clock": clock, "turns": c["turns"]},
                               ensure_ascii=False) + "\n")
    turns = sum(len(c["turns"]) for c in convs)
    print(f"wrote {a.out}: {len(convs)} conversations, {turns} turns", file=sys.stderr)


def cmd_measure(a):
    work = os.path.join(a.work, "measure")
    parts = run_parts(a.corpus, "measure", a.jobs, work)
    from lib import oracle
    oracle.ensure_env()
    exe = executable_total()
    covered = set()
    proposed, won = {}, {}
    nconv = nturn = 0
    for part in parts:
        inv = part["files"]
        for lid in part["cover"]:
            covered.add((inv[lid >> 20], lid & 0xFFFFF))
        for r in part["results"]:
            if r.get("error"):
                continue
            nconv += 1
            for w, props in r["turns"]:
                nturn += 1
                if w:
                    won[w] = won.get(w, 0) + 1
                for m in props:
                    proposed[m] = proposed.get(m, 0) + 1
    from lib import probes
    total = sum(len(v) for v in exe.values())
    hit = sum(len(v & {l for f, l in covered if f == p}) for p, v in exe.items()) if False else 0
    by_file = {}
    for f, l in covered:
        by_file.setdefault(f, set()).add(l)
    hit = sum(len(exe[p] & by_file.get(p, set())) for p in exe)
    by_dir = {}
    for p, lines in exe.items():
        d = os.path.relpath(p, probes.BOT_DIR).split(os.sep)[0] if os.sep in os.path.relpath(p, probes.BOT_DIR) else "."
        t, h = by_dir.get(d, (0, 0))
        by_dir[d] = (t + len(lines), h + len(lines & by_file.get(p, set())))
    # handlers: the modules that have a respond()
    from lib import oracle as orc
    handlers = []
    probes_mod = None
    import importlib.util  # noqa: F401
    moddir = os.path.join(probes.BOT_DIR, "modules")
    for fn in sorted(os.listdir(moddir)):
        if fn.endswith(".py") and fn != "__init__.py":
            src = open(os.path.join(moddir, fn), encoding="utf-8").read()
            if re.search(r"^def respond\(|^respond\s*=|^from .* import .*\brespond\b", src, re.M):
                handlers.append(fn[:-3])
    handlers = sorted(set(handlers) | set(proposed))
    cov = {"corpus": os.path.basename(a.corpus), "conversations": nconv, "turns": nturn,
           "lines_executable": total, "lines_covered": hit, "line_coverage_pct": round(100.0 * hit / total, 2),
           "by_package": {d: {"executable": t, "covered": h, "pct": round(100.0 * h / t, 1)} for d, (t, h) in sorted(by_dir.items())},
           "handlers": len(handlers), "proposed": len(proposed), "won": len(won),
           "never_proposed": sorted(set(handlers) - set(proposed)), "never_won": sorted(set(handlers) - set(won)),
           "proposals": dict(sorted(proposed.items())), "wins": dict(sorted(won.items()))}
    out = a.out or os.path.splitext(a.corpus)[0] + ".coverage.json"
    json.dump(cov, open(out, "w"), indent=1)
    print(f"coverage: {cov['line_coverage_pct']}% lines; proposed {cov['proposed']}/{cov['handlers']}, won {cov['won']}; "
          f"never proposed: {cov['never_proposed']}", file=sys.stderr)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd")
    for name in ("build", "mine", "measure", "_eval"):
        p = sub.add_parser(name)
        p.add_argument("--seed", type=int, default=0)
        p.add_argument("--out", default=None)
        p.add_argument("--split", default="dev", choices=["dev", "holdout", "all"],
                       help="which SmolTalkBench prompts to use (hold-out prompts are reserved for the grader)")
        p.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
        p.add_argument("--pool", type=int, default=70)
        p.add_argument("--corpus", default=os.path.join(HERE, "dev.jsonl"))
        p.add_argument("--work", default=os.path.join(os.environ.get("TMPDIR", "/tmp"), "corpus-work"))
        p.add_argument("--convs")
        p.add_argument("--part", type=int, default=0)
        p.add_argument("--mode", default="measure")
    argv = sys.argv[1:]
    if not argv or argv[0].startswith("-"):
        argv = ["build"] + argv
    a = ap.parse_args(argv)
    if a.cmd == "_eval":
        return eval_part(a.convs, a.part, a.out, a.mode)
    if a.cmd == "mine":
        return cmd_mine(a)
    if a.cmd == "measure":
        return cmd_measure(a)
    if a.out is None:
        a.out = os.path.join(HERE, "dev.jsonl") if a.seed == 0 else os.path.join(HERE, f"corpus-{a.seed}.jsonl")
    build(a)


if __name__ == "__main__":
    main()

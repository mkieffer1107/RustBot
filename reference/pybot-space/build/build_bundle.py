"""Run inside Pyodide (build.mjs): turn the bot's source zip into the browser bundle.

In:  /in/bot/**.py (the bot at one commit)
Out: /out/pybot.zip with
  - bot/**.pyc  sourceless bytecode for this Pyodide's CPython (imports skip compiling the source),
                co_filename kept as /pybot/bot/<path>.py so the trace still names the right files;
  - norm.json   every file's text, letters and digits only, for finding the file that holds a reply;
  - lines.json  for every file, how many of those letters and digits each line has, for turning a match
                in norm.json into a line number.
"""
import json
import os
import py_compile
import re
import zipfile

SRC, OUT = "/in", "/out"
keep = lambda s: re.sub(r"[^a-z0-9]", "", s.lower())
norm, lines = {}, {}
with zipfile.ZipFile(f"{OUT}/pybot.zip", "w", zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
    for dirpath, _, names in sorted(os.walk(f"{SRC}/bot")):
        for name in sorted(names):
            if not name.endswith(".py"):
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, SRC)
            pyc = py_compile.compile(path, cfile=f"/tmp/{rel}c", dfile=f"/pybot/{rel}", doraise=True,
                                     invalidation_mode=py_compile.PycInvalidationMode.UNCHECKED_HASH)
            bundle.write(pyc, rel + "c")
            with open(path, encoding="utf-8", errors="replace") as handle:
                text = handle.read()
            norm[rel] = keep(text)
            lines[rel] = [len(keep(line)) for line in text.splitlines(keepends=True)]
    bundle.writestr("norm.json", json.dumps(norm, separators=(",", ":")))
    bundle.writestr("lines.json", json.dumps(lines, separators=(",", ":")))
print(json.dumps({"files": len(norm), "bytes": os.path.getsize(f"{OUT}/pybot.zip")}))

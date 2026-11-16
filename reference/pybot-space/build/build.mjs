// Build the browser bundle inside the same Pyodide the page uses: node build.mjs <bot source zip> <build_bundle.py> <out zip>
import { loadPyodide } from "pyodide";
import { readFileSync, writeFileSync } from "node:fs";

const [src, script, out] = process.argv.slice(2);
const py = await loadPyodide();
py.unpackArchive(new Uint8Array(readFileSync(src)), "zip", { extractDir: "/in" });
py.FS.mkdirTree("/out");
py.runPython(readFileSync(script, "utf8"));
writeFileSync(out, py.FS.readFile("/out/pybot.zip"));
console.log("python", py.runPython("import sys; sys.version"), "->", out);

// Drives the Space page in headless Chromium: waits for the worker's "ready", sends the turns, prints JSON.
// usage: node space_drive.mjs URL '["turn", ...]'   (env CHROMIUM_PATH optional). Exit 3 = no browser driver/browser.
import fs from "node:fs";
import { execFileSync } from "node:child_process";

const [url, turnsJson] = process.argv.slice(2);
const turns = JSON.parse(turnsJson);

async function driver() {
  for (const name of ["playwright-core", "puppeteer-core"]) {
    try { return { name, mod: await import(name) }; } catch { /* try next */ }
  }
  return null;
}
function chromium() {
  const cands = [process.env.CHROMIUM_PATH, "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome", "/usr/bin/google-chrome-stable"];
  for (const n of ["chromium", "chromium-browser", "google-chrome"]) {
    try { cands.push(execFileSync("which", [n]).toString().trim()); } catch { /* absent */ }
  }
  return cands.find((p) => p && fs.existsSync(p));
}
const d = await driver();
const exe = chromium();
if (!d || !exe) {
  console.error(`no ${d ? "" : "playwright-core/puppeteer-core "}${exe ? "" : "Chromium executable (set CHROMIUM_PATH) "}`.trim());
  process.exit(3);
}
const launchArgs = ["--no-sandbox", "--disable-gpu"];
const browser = d.name === "playwright-core"
  ? await d.mod.chromium.launch({ executablePath: exe, headless: true, args: launchArgs })
  : await (d.mod.default || d.mod).launch({ executablePath: exe, headless: "new", args: launchArgs });
const page = await browser.newPage();
await page.evaluateOnNewDocument?.(() => {});
const init = () => {
  const Orig = window.Worker;
  window.__msgs = []; window.__t0 = performance.now();
  window.Worker = function (...a) {
    const w = new Orig(...a);
    window.__w = w;
    w.addEventListener("message", (e) => window.__msgs.push({ t: performance.now(), data: e.data }));
    return w;
  };
};
if (page.addInitScript) await page.addInitScript(init); else await page.evaluateOnNewDocument(init);
await page.goto(url, { waitUntil: "domcontentloaded" });
const deadline = Date.now() + 600000;
const wait = async (pred, what) => {
  while (Date.now() < deadline) {
    const r = await page.evaluate(pred);
    if (r) return r;
    await new Promise((res) => setTimeout(res, 50));
  }
  throw new Error("timeout waiting for " + what);
};
const ready = await wait(() => { const m = window.__msgs.find((x) => x.data.type === "ready" || x.data.type === "error"); return m ? { t: m.t, type: m.data.type, msg: m.data.message } : null; }, "ready");
if (ready.type !== "ready") { console.log(JSON.stringify({ error: ready.msg })); await browser.close(); process.exit(1); }
const replies = [];
for (let i = 0; i < turns.length; i++) {
  const sent = await page.evaluate(([i, text]) => { window.__sent = performance.now(); window.__w.postMessage({ type: "turn", id: 9000 + i, text }); return window.__sent; }, [i, turns[i]]);
  const m = await wait((id) => { const x = window.__msgs.find((y) => y.data.id === 9000 + id && (y.data.type === "reply" || y.data.type === "error")); return x ? { t: x.t, data: x.data } : null; }.bind(null, i), "reply " + i);
  replies.push({ ...m.data, wall_ms: m.t - sent });
}
console.log(JSON.stringify({ ready_s: ready.t / 1000, replies }));
await browser.close();

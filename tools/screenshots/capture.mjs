// Headless Chrome: load Diddle with the stand-in backend, play the scene,
// capture the README screenshots.
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
// Usage: npm run dev -- --port 1499 (in another shell), then
//   node tools/screenshots/capture.mjs docs/screenshots
// CHROME=/path/to/chrome and URL=http://localhost:1499/ override the defaults.
import { tmpdir } from "node:os";
const HERE = new URL(".", import.meta.url).pathname;
const OUT = process.argv[2] ?? "docs/screenshots";
const CHROME = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const URL_ = process.env.URL ?? "http://localhost:1499/";
mkdirSync(OUT, { recursive: true });
const W = 1500, H = 1700, SCALE = 2;
const chrome = spawn(CHROME,
  ["--headless=new", "--remote-debugging-port=9334", `--user-data-dir=${tmpdir()}/diddle-shots-${Date.now()}`, `--window-size=${W},${H}`, "about:blank"], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let targets;
for (let i = 0; i < 60; i++) { try { targets = await (await fetch("http://127.0.0.1:9334/json")).json(); break; } catch { await sleep(200); } }
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0; const pending = new Map(); const logs = [];
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); }
  if (d.method === "Runtime.consoleAPICalled" && d.params.type === "error") logs.push(d.params.args.map((a) => a.value ?? a.description).join(" "));
  if (d.method === "Runtime.exceptionThrown") logs.push(d.params.exceptionDetails.exception?.description ?? d.params.exceptionDetails.text);
};
const send = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
await send("Page.enable"); await send("Runtime.enable");
await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: SCALE, mobile: false });
await send("Page.addScriptToEvaluateOnNewDocument", { source: readFileSync(HERE + "mock.js", "utf8") });
await send("Page.navigate", { url: URL_ });
await sleep(4000);
const rx = readFileSync(HERE + "rx.json", "utf8");
const scene = readFileSync(HERE + "scene.js", "utf8");
const r = await send("Runtime.evaluate", { expression: `(${scene})(${rx})`, awaitPromise: true, returnByValue: true });
console.log("scene →", JSON.stringify(r.result?.result?.value ?? r.result?.exceptionDetails ?? r));
await sleep(800);
// Panel boxes, in CSS px.
const boxes = JSON.parse((await send("Runtime.evaluate", { returnByValue: true, expression: `JSON.stringify((() => {
  const sec = (t) => [...document.querySelectorAll("h2")].find((h) => h.textContent.trim().toUpperCase().startsWith(t))?.closest("section")?.getBoundingClientRect();
  const r = (b) => b && { x: b.left, y: b.top, w: b.width, h: b.height };
  return { wf: r(sec("WATERFALL")), rx: r(sec("RX DECODER")), entry: r(sec("ENTRY")), fkeys: r(sec("F-KEYS")),
    score: r(sec("SCORE")), log: r(sec("LOG")), bandmap: r(document.querySelector(".bandmap-col")?.getBoundingClientRect()),
    here: r(document.querySelector(".bandmap-col .here")?.getBoundingClientRect()) };
})())` })).result.result.value);
console.log(JSON.stringify(boxes));
const shot = async (name, x, y, w, h) => {
  const s = await send("Page.captureScreenshot", { format: "png", clip: { x, y, width: w, height: h, scale: 1 } });
  writeFileSync(`${OUT}/${name}.png`, Buffer.from(s.result.data, "base64"));
};
const pad = 8;
const B = boxes;
await shot("operating", 0, 0, W, B.log.y + B.log.h + 16);
await shot("waterfall-rx", B.wf.x - pad, B.wf.y - pad, B.wf.w + 2 * pad, B.rx.y + B.rx.h - B.wf.y + 2 * pad);
await shot("entry-stack", B.entry.x - pad, B.entry.y - pad, B.entry.w + 2 * pad, B.fkeys.y + B.fkeys.h - B.entry.y + 2 * pad);
// Score sits above the bandmap in the side column, so it and the log are
// separate shots.
await shot("score", B.score.x - pad, B.score.y - pad, B.score.w + 2 * pad, B.score.h + 2 * pad);
await shot("log", B.log.x - pad, B.log.y - pad, B.log.w + 2 * pad, B.log.h + 2 * pad);
// Bandmap: around our frequency, where the live spots are.
const bmTop = Math.max(B.bandmap.y, B.here.y - 300);
await shot("bandmap", B.bandmap.x - pad, bmTop - pad, B.bandmap.w + 2 * pad, 560 + 2 * pad);
if (logs.length) console.log("console errors:\n" + logs.slice(0, 10).join("\n"));
ws.close(); chrome.kill();

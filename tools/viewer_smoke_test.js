// Headless smoke test of viewer/index.html (no browser needed): runs the page script against a
// minimal DOM/canvas stub, exercises every control, and fails on exceptions or non-finite
// drawing coordinates. It checks UI logic only; visual layout must be checked in a browser.
const fs = require("fs");
const path = require("path");
const html = fs.readFileSync(path.join(__dirname, "..", "viewer", "index.html"), "utf8");
const m = html.match(/<script>([\s\S]*)<\/script>/);
if (!m) { console.error("no script block"); process.exit(1); }

let drawCalls = 0;
const geom = ["moveTo", "lineTo", "arc", "fillRect", "fillText", "rect"];
function makeCtx() {
  return new Proxy({}, {
    get(_, p) {
      if (p === "measureText") return () => ({ width: 10 });
      return (...a) => {
        drawCalls++;
        if (geom.includes(p)) {
          const nums = a.filter(v => typeof v === "number");
          for (const v of nums) if (!Number.isFinite(v)) throw new Error("non-finite arg to " + String(p) + ": " + JSON.stringify(a));
        }
      };
    },
    set() { return true; },
  });
}
const listeners = {};
const els = {};
function el(id) {
  if (els[id]) return els[id];
  const e = {
    id, value: "0", textContent: "", innerHTML: "", className: "", max: 0, width: 0, height: 0, clientWidth: 640, clientHeight: 400, tagName: "DIV",
    style: {}, children: [],
    addEventListener(t, f) { listeners[id + ":" + t] = f; },
    getContext() { return makeCtx(); },
    getBoundingClientRect() { return { left: 0, top: 0, width: 640, height: 400 }; },
    appendChild(c) { this.children.push(c); },
  };
  els[id] = e; return e;
}
const winL = {};
global.window = { devicePixelRatio: 1, addEventListener(t, f) { winL[t] = f; } };
global.document = { getElementById: el, createElement: () => ({ value: "", textContent: "" }), documentElement: {} };
global.getComputedStyle = () => ({ getPropertyValue: () => "" });
global.FileReader = class { readAsText(f) { this.result = f.text; this.onload(); } };
let intervals = [];
global.setInterval = (f) => { intervals.push(f); return intervals.length; };
global.clearInterval = () => { intervals = []; };

const run = new Function(m[1] + "\nreturn {load,synthetic,getD:()=>D};");
const api = run();
let fails = 0;
const ok = (c, msg) => { if (!c) { console.error("FAIL:", msg); fails++; } else console.log("ok  :", msg); };

// 1. synthetic demo through the real button path
listeners["demo:click"]();
ok(drawCalls > 200, "demo renders (" + drawCalls + " draw calls)");
ok(el("banner").textContent.includes("SYNTHETIC"), "synthetic banner shown");
const D = api.getD();
ok(D && D.frames.ticks.length === 120, "frames loaded");
// 2. every control
for (const mode of ["state", "amp", "phase"]) { el("mode").value = mode; listeners["mode:change"](); }
ok(true, "colour modes render");
el("cand").value = "1"; listeners["cand:change"]({ target: el("cand") });
ok(true, "candidate switch renders");
el("sl").value = "70"; listeners["sl:input"]({ target: el("sl") });
ok(el("prog").textContent.includes("71/120"), "slider seeks (" + el("prog").textContent + ")");
listeners["next:click"](); listeners["prev:click"](); listeners["prev:click"]();
listeners["play:click"](); ok(intervals.length === 1, "play starts timer"); intervals[0](); intervals[0]();
listeners["play:click"](); ok(intervals.length === 0, "pause clears timer");
listeners["tl:click"]({ clientX: 44 + (640 - 52) * 0.5, clientY: 50 });
ok(el("prog").textContent.length > 0, "timeline click seeks");
listeners["net:mousemove"]({ clientX: 100, clientY: 100 });
winL.keydown({ key: "ArrowRight", target: { tagName: "BODY" }, preventDefault() {} });
winL.keydown({ key: " ", target: { tagName: "BODY" }, preventDefault() {} }); winL.keydown({ key: " ", target: { tagName: "BODY" }, preventDefault() {} });
winL.resize();
// stats panel semantics
for (const f of [0, 45, 70, 100, 119]) { el("sl").value = String(f); listeners["sl:input"]({ target: el("sl") }); }
ok(el("stats").innerHTML.includes("not finality") || el("stats").innerHTML.includes("no"), "stats include observer-level wording");
// 3. file loading paths
const good = JSON.stringify(api.synthetic()).replace('"synthetic":true', '"synthetic":false');
listeners["file:change"]({ target: { files: [{ name: "x.trace.json", text: good }] } });
ok(!el("banner").textContent.includes("SYNTHETIC") && el("banner").textContent.includes("NOT global agreement"), "real-file banner carries the finality caveat");
listeners["file:change"]({ target: { files: [{ name: "bad.json", text: "{nope" }] } });
ok(el("banner").textContent.includes("Could not parse"), "invalid JSON reported");
listeners["file:change"]({ target: { files: [{ name: "w.json", text: JSON.stringify({ schema: "other" }) }] } });
ok(el("banner").textContent.includes("Not a crqisl-trace/1"), "wrong schema reported");
const nf = api.synthetic(); nf.frames = null;
listeners["file:change"]({ target: { files: [{ name: "nf.json", text: JSON.stringify(nf) }] } });
ok(el("banner").textContent.includes("--trace"), "missing frames explains how to record them");
// 4. NaN / null robustness (engine writes null for non-finite numbers)
const nanTrace = api.synthetic(); nanTrace.frames.amp[10][3] = null; nanTrace.frames.phase[10][3] = null; nanTrace.meta.synthetic = false;
listeners["file:change"]({ target: { files: [{ name: "n.json", text: JSON.stringify(nanTrace) }] } });
el("sl").value = "10"; listeners["sl:input"]({ target: el("sl") });
ok(true, "null amplitude/phase values do not break rendering");
console.log(fails ? ("\n" + fails + " FAILED") : "\nall viewer smoke checks passed");
process.exit(fails ? 1 : 0);

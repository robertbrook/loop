import assert from "node:assert/strict";
import { LIMITS } from "../web/limits.js";
import { formatEvaluations } from "../web/results.js";

class Element {
  children = [];
  listeners = {};
  value = "";
  textContent = "";
  disabled = false;
  scrollHeight = 0;
  append(...children) {
    for (const child of children) child.parent = this;
    this.children.push(...children);
  }
  replaceChildren(...children) { this.children = []; this.append(...children); }
  get firstElementChild() { return this.children[0]; }
  remove() { this.parent.children.splice(this.parent.children.indexOf(this), 1); }
  addEventListener(type, listener) { this.listeners[type] = listener; }
  fire(type, event = {}) { return this.listeners[type]({ preventDefault() {}, ...event }); }
}

const elements = new Map([
  "editor", "results", "example-list", "run", "repl-run", "runtime-status",
  "status-dot", "line-numbers", "repl", "repl-form", "reset", "open-files", "source-files",
].map((id) => [id, new Element()]));
globalThis.document = {
  getElementById: (id) => {
    assert.ok(elements.has(id), `Unexpected element: ${id}`);
    return elements.get(id);
  },
  createElement: () => new Element(),
};
const $ = (id) => elements.get(id);
const timers = new Map();
let nextTimer = 0;
globalThis.setTimeout = (callback, delay) => {
  timers.set(++nextTimer, { callback, delay });
  return nextTimer;
};
globalThis.clearTimeout = (id) => timers.delete(id);
function expire(delay) {
  const timer = [...timers.entries()].find(([, timer]) => timer.delay === delay);
  assert.ok(timer, `Missing ${delay}ms timer`);
  timers.delete(timer[0]);
  timer[1].callback();
}

const workers = [];
let failConstruction = false;
globalThis.Worker = class {
  terminated = false;
  messages = [];
  constructor() {
    if (failConstruction) throw new Error("Worker creation blocked");
    workers.push(this);
  }
  terminate() { this.terminated = true; }
  postMessage(message) { this.messages.push(message); }
  emit(data) { this.onmessage({ data: { inputLimit: 65_536, sourceLimit: 128, ...data } }); }
  fail() { this.onerror({ preventDefault() {}, message: "Worker crashed" }); }
};
const latest = () => workers.at(-1);
function state(text, disabled) {
  assert.equal($("runtime-status").textContent, text);
  assert.equal($("run").disabled, disabled);
  assert.equal($("repl-run").disabled, disabled);
  assert.equal($("open-files").disabled, disabled);
}
function lastText() { return $("results").children.at(-1).children[1].textContent; }
function run() { $("run").fire("click"); }
function submit(source) { $("repl").value = source; $("repl-form").fire("submit"); }

await import("../web/app.js");
state("Loading…", true);
assert.equal($("example-list").children.length, 5);
run();
assert.equal(latest().messages.length, 0);
latest().emit({ type: "ready" });
state("Ready", false);
assert.equal(timers.size, 0);
run();
state("Running…", true);
assert.equal($("run").textContent, "Running…");
run();
submit("(+ 1 2)");
assert.equal(latest().messages.length, 1);
assert.equal($("repl").value, "(+ 1 2)");
latest().emit({
  type: "result", evaluations: [{ output: "hello", value: "()" }, { output: "", value: "3" }],
  error: null, elapsed: 1,
});
state("Ready", false);
assert.equal(lastText(), "hello()\n3");
submit("(+ 1 2)");
assert.equal($("repl").value, "");
latest().emit({ type: "result", evaluations: [], error: "Example error", elapsed: 1 });
state("Ready", false);
assert.equal(lastText(), "Example error");
$("repl").fire("keydown", { key: "ArrowUp" });
assert.equal($("repl").value, "(+ 1 2)");
assert.equal(timers.size, 0);

run();
const timedOut = latest();
expire(LIMITS.evaluationTimeoutMs);
assert.ok(timedOut.terminated);
state("Loading…", true);
assert.match(lastText(), /Execution stopped/);
timedOut.emit({ type: "ready" });
timedOut.fail();
state("Loading…", true);
latest().emit({ type: "ready" });
run();
const trapped = latest();
trapped.emit({ type: "fatal", text: "WASM trap" });
assert.ok(trapped.terminated);
state("Loading…", true);
assert.match(lastText(), /WASM trap.*\n.*definitions were cleared/);
latest().emit({ type: "load-error", text: "Network unavailable" });
state("Could not load. Reset session to retry.", true);
assert.ok(latest().terminated);
latest().emit({ type: "ready" });
state("Could not load. Reset session to retry.", true);
$("reset").fire("click");
expire(LIMITS.loadTimeoutMs);
state("Loading timed out. Reset session to retry.", true);
assert.ok(latest().terminated);
latest().emit({ type: "ready" });
state("Loading timed out. Reset session to retry.", true);
$("reset").fire("click");
latest().fail();
state("Runtime stopped. Reset session to retry.", true);
failConstruction = true;
$("reset").fire("click");
state("Could not load. Reset session to retry.", true);
assert.equal(lastText(), "Worker creation blocked");
failConstruction = false;
$("reset").fire("click");
latest().emit({ type: "ready" });
state("Ready", false);
assert.equal(timers.size, 0);
for (let i = 0; i < LIMITS.resultEntries + 2; i++) {
  submit(String(i));
  latest().emit({ type: "result", evaluations: [{ value: String(i), output: "" }], error: null, elapsed: 0 });
}
assert.equal($("results").children.length, LIMITS.resultEntries);
assert.equal(formatEvaluations([{ output: " \n", value: "()" }, { output: "\n", value: '"λ"' }]), ' \n()\n\n"λ"');
assert.equal(formatEvaluations([]), "");

function file(name, source) {
  const bytes = new TextEncoder().encode(source);
  return { name, size: bytes.length, async arrayBuffer() { return bytes.buffer; } };
}
await $("source-files").fire("change", { target: { files: [
  file("main.loop", '(load "math.loop") (import math square) (square 12)'),
  file("math.loop", "(module math (export square) (define (square n) (* n n)))"),
] } });
state("Ready", false);
assert.equal($("example-list").children.length, 7);
assert.equal($("example-list").value, "file:main.loop");
assert.match($("editor").value, /load "math.loop"/);
run();
assert.equal(latest().messages.at(-1).name, "main.loop");
assert.equal(latest().messages.at(-1).sources.length, 2);
latest().emit({ type: "result", evaluations: [{ value: "144", output: "" }], error: null });
$("editor").value = "(+ 2 2)";
$("example-list").fire("change", { target: { value: "file:math.loop" } });
assert.match($("editor").value, /module math/);
$("example-list").fire("change", { target: { value: "file:main.loop" } });
assert.equal($("editor").value, "(+ 2 2)", "switching files preserves edits");
submit("(+ 3 3)");
assert.equal(latest().messages.at(-1).name, "<repl>");
assert.equal(latest().messages.at(-1).sources[0][1], "(+ 2 2)");
latest().emit({ type: "result", evaluations: [{ value: "6", output: "" }], error: null });
await $("source-files").fire("change", { target: { files: [
  { name: "huge.loop", size: 65_537, arrayBuffer() { assert.fail("oversized files must not be read"); } },
] } });
assert.match(lastText(), /input limit/);
state("Ready", false);
await $("source-files").fire("change", { target: { files: [
  { name: "binary.loop", size: 1, async arrayBuffer() { return Uint8Array.of(255).buffer; } },
] } });
assert.match(lastText(), /valid|encoded|encoding/i);
await $("source-files").fire("change", { target: { files: [file("x", "1"), file("x", "2")] } });
assert.match(lastText(), /Duplicate filename/);
$("reset").fire("click");
latest().emit({ type: "ready" });
run();
assert.equal(latest().messages.at(-1).sources.length, 2, "reset retains explicitly opened files, not definitions");
latest().emit({ type: "result", evaluations: [{ value: "4", output: "" }], error: null });
let finishReading;
const opening = $("source-files").fire("change", { target: { files: [{
  name: "late.loop", size: 1, arrayBuffer() { return new Promise((resolve) => { finishReading = resolve; }); },
}] } });
state("Opening…", true);
$("reset").fire("click");
finishReading(new TextEncoder().encode("1").buffer);
await opening;
state("Loading…", true);
assert.equal($("example-list").children.length, 7, "stale reads must not replace the editor");
console.log("Browser runtime state and presentation checks passed.");

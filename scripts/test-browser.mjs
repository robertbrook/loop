import assert from "node:assert/strict";
import { LIMITS } from "../web/limits.js";
import { formatEvaluations } from "../web/results.js";
import { completionToken } from "../web/completion.js";

class Element {
  children = [];
  listeners = {};
  value = "";
  textContent = "";
  disabled = false;
  attributes = {};
  setAttribute(name, value) { this.attributes[name] = value; }
  removeAttribute(name) { delete this.attributes[name]; }
  selectionStart = 0;
  selectionEnd = 0;
  focus() { document.activeElement = this; }
  setRangeText(text, start, end) {
    this.value = this.value.slice(0, start) + text + this.value.slice(end);
    this.selectionStart = this.selectionEnd = start + text.length;
  }
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
  "status-dot", "line-numbers", "repl", "repl-form", "reset", "open-files", "save-file", "source-files", "repl-suggestions",
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
  emit(data) {
    const registeredSources = this.messages.at(-1)?.sources.map(([name]) => name) ?? [];
    this.onmessage({ data: { inputLimit: 65_536, sourceLimit: 128, registeredSources, symbols: [], ...data } });
  }
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
assert.equal(latest().messages.at(-1).sources.length, 1, "only the edited file is synchronized");
latest().emit({ type: "result", evaluations: [{ value: "6", output: "" }], error: null });
submit("(+ 4 4)");
assert.deepEqual(latest().messages.at(-1).sources, [], "unchanged files must not be sent again");
latest().emit({ type: "result", evaluations: [], error: "A language error after registration" });
submit("(+ 5 5)");
assert.deepEqual(latest().messages.at(-1).sources, [], "language errors do not invalidate synchronization");
latest().emit({ type: "result", evaluations: [{ value: "10", output: "" }], error: null });
$("editor").value = "(+ 6 6)";
run();
assert.deepEqual(latest().messages.at(-1).sources, [["main.loop", "(+ 6 6)"]]);
$("editor").value = "(+ 7 7)";
latest().emit({ type: "result", evaluations: [{ value: "12", output: "" }], error: null });
run();
assert.deepEqual(latest().messages.at(-1).sources, [["main.loop", "(+ 7 7)"]],
  "edits during a run must not be marked as synchronized");
latest().emit({ type: "result", evaluations: [{ value: "14", output: "" }], error: null });
await $("source-files").fire("change", { target: { files: [file("math.loop", "(define updated 1)")] } });
run();
assert.deepEqual(latest().messages.at(-1).sources, [["math.loop", "(define updated 1)"]],
  "reopening a file synchronizes its replacement");
latest().emit({ type: "result", evaluations: [{ value: "updated", output: "" }], error: null });
$("editor").value = "(define updated 2)";
$("example-list").fire("change", { target: { value: "file:main.loop" } });
$("editor").value = "(+ 8 8)";
run();
assert.deepEqual(latest().messages.at(-1).sources.map(([name]) => name), ["main.loop", "math.loop"]);
latest().emit({ type: "result", registeredSources: ["main.loop"], evaluations: [], error: "math.loop registration failed" });
run();
assert.deepEqual(latest().messages.at(-1).sources, [["math.loop", "(define updated 2)"]],
  "only unacknowledged files are retried after a partial registration failure");
latest().emit({ type: "result", evaluations: [{ value: "16", output: "" }], error: null });
run();
assert.deepEqual(latest().messages.at(-1).sources, []);
const oldWorker = latest();
oldWorker.emit({ type: "fatal", text: "WASM trap" });
latest().emit({ type: "ready" });
run();
assert.equal(latest().messages.at(-1).sources.length, 2, "worker replacement requires full synchronization");
oldWorker.emit({ type: "result", evaluations: [], error: null });
state("Running…", true);
latest().emit({ type: "result", evaluations: [{ value: "16", output: "" }], error: null });
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
run();
assert.deepEqual(latest().messages.at(-1).sources, [], "full synchronization is acknowledged after reset");
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

assert.deepEqual(completionToken("(str)", 4), { from: 1, to: 4, prefix: "str" });
assert.deepEqual(completionToken("(string-ref)", 4), { from: 1, to: 11, prefix: "str" });
assert.deepEqual(completionToken("'λ", 2), { from: 1, to: 2, prefix: "λ" });
for (const source of ['"str', '"escaped \\" str', "; str", "(123", "(#t", "(+1", "( ", ""]) {
  assert.equal(completionToken(source, source.length), null, source);
}
assert.equal(completionToken("(str)", 1, 4), null, "selection must not be overwritten");
assert.equal(completionToken('";" str', 7).prefix, "str");
latest().emit({ type: "ready", symbols: ["string-ref", "string-length", "square", "λ-function"] });
const repl = $("repl"), suggestions = $("repl-suggestions");
function typeRepl(source, caret = source.length) {
  repl.focus();
  repl.value = source;
  repl.selectionStart = repl.selectionEnd = caret;
  repl.fire("input");
}
function key(key, extra = {}) {
  let prevented = false;
  repl.fire("keydown", { key, preventDefault() { prevented = true; }, ...extra });
  return prevented;
}
typeRepl("(str)");
// Move inside the token; the closing parenthesis must survive completion.
typeRepl("(str)", 4);
assert.equal(suggestions.hidden, false);
assert.deepEqual(suggestions.children.map(option => option.textContent), ["string-length", "string-ref"]);
assert.equal(repl.attributes["aria-expanded"], "true");
assert.equal(key("ArrowDown"), true);
assert.equal(suggestions.children[1].attributes["aria-selected"], "true");
assert.equal(key("Tab"), true);
assert.equal(repl.value, "(string-ref)");
assert.equal(repl.selectionStart, 11);
assert.equal(suggestions.hidden, true);
typeRepl("(sq");
assert.equal(key("Enter"), true);
assert.equal(repl.value, "(square");
assert.equal(latest().messages.length, 0, "accepting completion does not evaluate");
typeRepl("(def");
assert.equal(suggestions.children[0].textContent, "define");
assert.equal(key("Escape"), true);
assert.equal(suggestions.hidden, true);
assert.equal(repl.value, "(def");
typeRepl("(λ");
suggestions.children[0].fire("click");
assert.equal(repl.value, "(λ-function");
typeRepl('"str');
assert.equal(suggestions.hidden, true);
typeRepl("(str");
repl.fire("compositionstart");
assert.equal(suggestions.hidden, true);
assert.equal(key("Enter", { isComposing: true }), false);
repl.fire("compositionend");
assert.equal(suggestions.hidden, false);
repl.fire("blur");
assert.equal(suggestions.hidden, true);
typeRepl("(str");
$("reset").fire("click");
assert.equal(suggestions.hidden, true);
latest().emit({ type: "ready", symbols: ["abs"] });
typeRepl("(sq");
assert.equal(suggestions.hidden, true, "reset removes user definitions from suggestions");
typeRepl("(ab");
assert.equal(suggestions.children[0].textContent, "abs");
assert.equal(key("Tab", { shiftKey: true }), false, "Shift+Tab retains focus navigation");
key("Escape");
assert.equal(key("ArrowUp"), true, "history is available when suggestions are closed");
submit('(write-file "output.json" "{}")');
latest().emit({ type: "file-written", name: "output.json", source: "{}" });
state("Running…", true);
assert.equal(timers.size, 1, "file notifications must not cancel the execution deadline");
latest().emit({ type: "result", evaluations: [{ value: "()", output: "" }], error: null });
$("example-list").fire("change", { target: { value: "file:output.json" } });
assert.equal($("editor").value, "{}");
run();
assert.ok(latest().messages.at(-1).sources.some(([name, contents]) => name === "output.json" && contents === "{}"));
latest().emit({ type: "file-written", name: "output.json", source: '{"written":true}' });
assert.equal($("editor").value, '{"written":true}', "writes update an unchanged editor buffer");
$("editor").value = '{"draft":true}';
latest().emit({ type: "file-written", name: "output.json", source: '{"written":false}' });
assert.equal($("editor").value, '{"draft":true}', "writes must not discard edits made during evaluation");
latest().emit({ type: "result", evaluations: [{ value: "()", output: "" }], error: null });
console.log("Browser runtime state and presentation checks passed.");

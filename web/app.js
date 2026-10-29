import { LIMITS } from "./limits.js";
import { formatEvaluations } from "./results.js";

const examples = [
  { name: "Hello, Lisp", source: `; Everything starts with an expression.
(+ 1 2 3)

; Put expressions inside expressions.
(* (+ 2 3) (- 10 4))

; Give an idea a name.
(define answer 42)
(* answer 2)` },
  { name: "Make a function", source: `; Name a function.
(define (square x)
  (* x x))

(square 12)

; Functions can return other functions.
(define (add-by n)
  (lambda (x) (+ x n)))

(define add-ten (add-by 10))
(add-ten 32)` },
  { name: "Lists & patterns", source: `; A quote keeps a list as data.
(define numbers '(1 2 3 4 5 6))

; Transform every item.
(map (lambda (x) (* x x)) numbers)

; Keep only what matches.
(filter (lambda (x) (= (modulo x 2) 0))
        numbers)

; Bring the list together.
(apply + numbers)` },
  { name: "Think recursively", source: `; Factorial: n × (n-1) × ... × 1
(define (factorial n)
  (if (<= n 1)
      1
      (* n (factorial (- n 1)))))

(factorial 6)

; Try a different input.
(factorial 10)` },
  { name: "Choose a path", source: `; Local names, one small decision.
(let ((temperature 18)
      (threshold 20))
  (if (< temperature threshold)
      "Bring a sweater."
      "Leave the sweater." ))

; Conditions can be combined.
(and (> 10 5) (< 10 20))
(or #f 'a-little-optimism)` },
];

const $ = (id) => document.getElementById(id);
const editor = $("editor");
const results = $("results");
let worker, timer, runCount = 0, currentLabel = "";
let runtimeState = "loading";
let history = [], historyPosition = 0;
const sourceFiles = new Map();
let sourceName = "<program>", inputLimit, sourceLimit;

function saveSource() {
  if (sourceFiles.has(sourceName)) sourceFiles.set(sourceName, editor.value);
}

function updateLines() {
  $("line-numbers").textContent = Array.from({ length: editor.value.split("\n").length }, (_, i) => i + 1).join("\n");
}

function selectExample(index) {
  saveSource();
  sourceName = "<program>";
  editor.value = examples[index].source;
  $("example-list").value = String(index);
  updateLines();
}

examples.forEach((example, i) => {
  const option = document.createElement("option");
  option.value = String(i);
  option.textContent = example.name;
  $("example-list").append(option);
});
$("example-list").addEventListener("change", (event) => {
  const selected = event.target.value;
  if (!selected.startsWith("file:")) {
    selectExample(Number(selected));
    return;
  }
  saveSource();
  sourceName = selected.slice(5);
  editor.value = sourceFiles.get(sourceName);
  updateLines();
});
$("open-files").addEventListener("click", () => $("source-files").click());
$("source-files").addEventListener("change", async (event) => {
  const files = [...event.target.files];
  event.target.value = "";
  if (!files.length || runtimeState !== "ready") return;
  const instance = worker;
  setRuntimeState("running", "Opening…");
  try {
    const names = new Set(sourceFiles.keys());
    const selected = new Set();
    for (const file of files) {
      if (selected.has(file.name)) throw new Error(`Duplicate filename: ${file.name}. Open files with distinct names.`);
      selected.add(file.name);
      names.add(file.name);
      if (file.size > inputLimit) throw new Error(`${file.name}: program exceeds the 64 KiB input limit.`);
    }
    if (names.size > sourceLimit) throw new Error(`Open at most ${sourceLimit} source files per session.`);
    const loaded = await Promise.all(files.map(async (file) => [
      file.name, new TextDecoder("utf-8", { fatal: true }).decode(await file.arrayBuffer()),
    ]));
    if (instance !== worker) return;
    saveSource();
    for (const [name, source] of loaded) {
      if (!sourceFiles.has(name)) {
        const option = document.createElement("option");
        option.value = `file:${name}`;
        option.textContent = name;
        $("example-list").append(option);
      }
      sourceFiles.set(name, source);
    }
    sourceName = loaded[0][0];
    editor.value = sourceFiles.get(sourceName);
    $("example-list").value = `file:${sourceName}`;
    updateLines();
    setRuntimeState("ready", "Ready");
  } catch (error) {
    if (instance !== worker) return;
    setRuntimeState("ready", "Ready");
    addResult(error.message, true, "Open files");
  }
});

function setRuntimeState(state, text) {
  runtimeState = state;
  $("run").disabled = state !== "ready";
  $("repl-run").disabled = state !== "ready";
  $("open-files").disabled = state !== "ready";
  $("run").textContent = state === "running" ? "Running…" : "Run";
  $("runtime-status").textContent = text;
  const color = state === "ready" || state === "running" ? "ready" : state === "failed" ? "error" : "";
  $("status-dot").className = `status-dot ${color}`;
}

function addResult(text, failed, label, elapsed) {
  const entry = document.createElement("div");
  entry.className = `result-entry${failed ? " error" : ""}`;
  const heading = document.createElement("div");
  heading.className = "result-label";
  const title = document.createElement("span");
  title.textContent = failed ? `${label} · error` : label;
  const time = document.createElement("span");
  time.textContent = elapsed === undefined ? "" : `${elapsed.toFixed(1)} ms`;
  heading.append(title, time);
  const value = document.createElement("pre");
  value.textContent = text || "No expressions to evaluate.";
  entry.append(heading, value);
  results.append(entry);
  while (results.children.length > LIMITS.resultEntries) results.firstElementChild.remove();
  results.scrollTop = results.scrollHeight;
}

function failRuntime(status, detail) {
  clearTimeout(timer);
  worker?.terminate();
  worker = undefined;
  setRuntimeState("failed", status);
  addResult(detail, true, "Runtime");
}

function startRuntime() {
  clearTimeout(timer);
  worker?.terminate();
  worker = undefined;
  setRuntimeState("loading", "Loading…");
  let instance;
  try {
    instance = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  } catch (error) {
    failRuntime("Could not load. Reset session to retry.", error.message);
    return;
  }
  worker = instance;
  timer = setTimeout(() => {
    failRuntime("Loading timed out. Reset session to retry.",
      "The runtime could not load. Check your connection, then reset the session.");
  }, LIMITS.loadTimeoutMs);
  instance.onmessage = ({ data }) => {
    if (instance !== worker) return;
    clearTimeout(timer);
    if (data.type === "ready") {
      inputLimit = data.inputLimit;
      sourceLimit = data.sourceLimit;
      setRuntimeState("ready", "Ready");
    } else if (data.type === "result") {
      setRuntimeState("ready", "Ready");
      addResult(data.error ?? formatEvaluations(data.evaluations), data.error !== null, currentLabel, data.elapsed);
    } else if (data.type === "fatal") {
      addResult(`${data.text}\nThe runtime was restarted; definitions were cleared.`, true, currentLabel);
      startRuntime();
    } else if (data.type === "load-error") {
      failRuntime("Could not load. Reset session to retry.", data.text);
    }
  };
  instance.onerror = (event) => {
    event.preventDefault();
    if (instance !== worker) return;
    failRuntime("Runtime stopped. Reset session to retry.",
      event.message || "Unexpected worker error. Reset the session to retry.");
  };
}

function evaluate(source, label, name = sourceName) {
  if (runtimeState !== "ready") return;
  if (!source.trim()) {
    addResult("Write an expression first, such as (+ 1 2).", true, label);
    return;
  }
  setRuntimeState("running", "Running…");
  currentLabel = label;
  saveSource();
  worker.postMessage({ type: "evaluate", source, name, sources: [...sourceFiles] });
  timer = setTimeout(() => {
    addResult(`Execution stopped after ${LIMITS.evaluationTimeoutMs / 1000} seconds. Definitions were cleared; try a smaller program.`, true, currentLabel);
    startRuntime();
  }, LIMITS.evaluationTimeoutMs);
}

$("run").addEventListener("click", () => evaluate(editor.value, `Program ${++runCount}`));
editor.addEventListener("input", updateLines);
editor.addEventListener("scroll", () => { $("line-numbers").scrollTop = editor.scrollTop; });
editor.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    evaluate(editor.value, `Program ${++runCount}`);
  } else if (event.key === "Tab") {
    event.preventDefault();
    editor.setRangeText("  ", editor.selectionStart, editor.selectionEnd, "end");
    updateLines();
  }
});
$("repl-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const source = $("repl").value;
  if (runtimeState !== "ready") return;
  evaluate(source, `λ ${source}`, "<repl>");
  if (source.trim()) {
    history.push(source);
    historyPosition = history.length;
    $("repl").value = "";
  }
});
$("repl").addEventListener("keydown", (event) => {
  if (!["ArrowUp", "ArrowDown"].includes(event.key)) return;
  event.preventDefault();
  historyPosition = Math.max(0, Math.min(history.length, historyPosition + (event.key === "ArrowUp" ? -1 : 1)));
  $("repl").value = history[historyPosition] || "";
});
$("reset").addEventListener("click", () => {
  results.replaceChildren();
  runCount = 0;
  addResult("Definitions cleared.", false, "Session reset");
  startRuntime();
});
selectExample(0);
startRuntime();

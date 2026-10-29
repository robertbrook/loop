const examples = [
  { name: "Hello, Lisp", symbol: "(+)", caption: "Start small. Change a number.", source: `; Everything starts with an expression.
(+ 1 2 3)

; Put expressions inside expressions.
(* (+ 2 3) (- 10 4))

; Give an idea a name.
(define answer 42)
(* answer 2)` },
  { name: "Make a function", symbol: "λ", caption: "A function is a value, too.", source: `; Name a function.
(define (square x)
  (* x x))

(square 12)

; Functions can return other functions.
(define (add-by n)
  (lambda (x) (+ x n)))

(define add-ten (add-by 10))
(add-ten 32)` },
  { name: "Lists & patterns", symbol: "′()", caption: "One operation. A whole list.", source: `; A quote keeps a list as data.
(define numbers '(1 2 3 4 5 6))

; Transform every item.
(map (lambda (x) (* x x)) numbers)

; Keep only what matches.
(filter (lambda (x) (= (modulo x 2) 0))
        numbers)

; Bring the list together.
(apply + numbers)` },
  { name: "Think recursively", symbol: "↻", caption: "A problem, made a little smaller.", source: `; Factorial: n × (n-1) × ... × 1
(define (factorial n)
  (if (<= n 1)
      1
      (* n (factorial (- n 1)))))

(factorial 6)

; Try a different input.
(factorial 10)` },
  { name: "Choose a path", symbol: "?", caption: "Only #f is false. Every other value is true.", source: `; Local names, one small decision.
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
let worker, timer, busy = false, ready = false, runCount = 0, currentLabel = "";
let selected = 0;
let history = [], historyPosition = 0;

function updateLines() {
  $("line-numbers").textContent = Array.from({ length: editor.value.split("\n").length }, (_, i) => i + 1).join("\n");
}

function selectExample(index) {
  selected = index;
  editor.value = examples[index].source;
  $("example-caption").textContent = examples[index].caption;
  document.querySelectorAll(".example").forEach((button, i) => button.setAttribute("aria-current", String(i === index)));
  updateLines();
}

examples.forEach((example, i) => {
  const button = document.createElement("button");
  button.className = "example";
  const name = document.createElement("span");
  name.textContent = example.name;
  const symbol = document.createElement("span");
  symbol.textContent = example.symbol;
  symbol.setAttribute("aria-hidden", "true");
  button.append(name, symbol);
  button.addEventListener("click", () => selectExample(i));
  $("example-list").append(button);
});

function controls() {
  $("run").disabled = !ready || busy;
  $("repl-run").disabled = !ready || busy;
  $("run").firstChild.textContent = busy ? "Running… " : "Run program ";
}

function status(text, state = "") {
  $("runtime-status").textContent = text;
  $("status-dot").className = `status-dot ${state}`;
}

function addResult(text, failed, label, elapsed) {
  results.querySelector(".empty-result")?.remove();
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
  while (results.children.length > 40) results.firstElementChild.remove();
  results.scrollTop = results.scrollHeight;
}

function startRuntime() {
  clearTimeout(timer);
  worker?.terminate();
  ready = false;
  busy = false;
  controls();
  status("Loading WebAssembly…");
  const instance = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  worker = instance;
  timer = setTimeout(() => {
    instance.terminate();
    status("Loading timed out. Reset session to retry.", "error");
    addResult("The runtime could not load. Check your connection, then reset the session.", true, "Runtime");
  }, 15000);
  instance.onmessage = ({ data }) => {
    if (instance !== worker) return;
    clearTimeout(timer);
    if (data.type === "ready") {
      ready = true;
      status("WebAssembly ready · runs locally", "ready");
    } else if (data.type === "result") {
      busy = false;
      addResult(data.text, data.failed, currentLabel, data.elapsed);
    } else if (data.type === "fatal") {
      addResult(`${data.text}\nThe runtime was restarted; definitions were cleared.`, true, currentLabel);
      startRuntime();
    } else if (data.type === "load-error") {
      instance.terminate();
      ready = false;
      busy = false;
      status("Could not load. Reset session to retry.", "error");
      addResult(data.text, true, "Runtime");
    }
    controls();
  };
  instance.onerror = (event) => {
    event.preventDefault();
    clearTimeout(timer);
    instance.terminate();
    ready = false;
    busy = false;
    status("Runtime stopped. Reset session to retry.", "error");
    addResult(event.message || "Unexpected worker error. Reset the session to retry.", true, "Runtime");
    controls();
  };
}

function evaluate(source, label) {
  if (!ready || busy) return;
  if (!source.trim()) {
    addResult("Write an expression first, such as (+ 1 2).", true, label);
    return;
  }
  busy = true;
  currentLabel = label;
  controls();
  worker.postMessage({ type: "evaluate", source });
  timer = setTimeout(() => {
    addResult("Execution stopped after 3 seconds. Definitions were cleared; try a smaller program.", true, currentLabel);
    startRuntime();
  }, 3000);
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
  if (!ready || busy) return;
  evaluate(source, `λ ${source}`);
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
  addResult("Fresh session. Your editor is unchanged; definitions and results have been cleared.", false, "Session reset");
  startRuntime();
});
selectExample(selected);
startRuntime();

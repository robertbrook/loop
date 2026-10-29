import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { formatEvaluations } from "../web/results.js";

const { instance } = await WebAssembly.instantiate(await readFile(new URL("../dist/loop.wasm", import.meta.url)));
const runtime = instance.exports;
function execute(source) {
  const bytes = new TextEncoder().encode(source);
  const pointer = runtime.input_buffer(bytes.length);
  new Uint8Array(runtime.memory.buffer, pointer, bytes.length).set(bytes);
  const failed = runtime.evaluate() !== 0;
  const result = JSON.parse(new TextDecoder().decode(new Uint8Array(runtime.memory.buffer, runtime.output_pointer(), runtime.output_length())));
  assert.equal(failed, result.error !== null);
  assert.ok(Array.isArray(result.evaluations));
  return result;
}

function evaluate(source) {
  const result = execute(source);
  return { failed: result.error !== null, text: result.error ?? formatEvaluations(result.evaluations) };
}

assert.deepEqual(execute('(display "hello") (+ 1 2) (begin (display "bye") (newline) 4)'), {
  evaluations: [
    { value: "()", output: "hello" },
    { value: "3", output: "" },
    { value: "4", output: "bye\n" },
  ],
  error: null,
});
assert.equal(evaluate('(display "hello") (+ 1 2) (begin (display "bye") (newline) 4)').text, "hello()\n3\nbye\n4");
assert.deepEqual(execute("; no expressions"), { evaluations: [], error: null });
assert.deepEqual(execute('(display "discard on error") missing'), {
  evaluations: [], error: "Unknown symbol: missing\n  at <input>:1:30",
});
const special = 'a"\\\n\t\r\0\u0001\u0008\u000cλ😀';
const lispString = `"${special.replaceAll("\\", "\\\\").replaceAll('"', '\\"').replaceAll("\n", "\\n").replaceAll("\t", "\\t")}"`;
assert.deepEqual(execute(`(display ${lispString})`), {
  evaluations: [{ value: "()", output: special }], error: null,
});
const inputLimit = runtime.input_limit();
assert.equal(runtime.input_buffer(inputLimit + 1), 0);
assert.deepEqual(execute(" ".repeat(inputLimit)), { evaluations: [], error: null });
assert.deepEqual(execute("é".repeat(inputLimit / 2)), {
  evaluations: [], error: `Unknown symbol: ${"é".repeat(inputLimit / 2)}\n  at <input>:1:1`,
});
assert.deepEqual(evaluate("(+ 1 2 3)"), { failed: false, text: "6" });
assert.deepEqual(evaluate("(define (factorial n) (if (<= n 1) 1 (* n (factorial (- n 1))))) (factorial 10)"), { failed: false, text: "factorial\n3628800" });
assert.deepEqual(evaluate("(factorial 5)"), { failed: false, text: "120" });
assert.deepEqual(evaluate("(map (lambda (x) (* x x)) '(1 2 3))"), { failed: false, text: "(1 4 9)" });
assert.deepEqual(evaluate("(let ((x 4)) (+ x 2))"), { failed: false, text: "6" });
assert.deepEqual(evaluate("(/ 1 0)"), { failed: true, text: "Division by zero\n  at <input>:1:1 in /" });
assert.deepEqual(evaluate("(+ 8 9)"), { failed: false, text: "17" });
assert.equal(evaluate("(define (forever) (forever)) (forever)").failed, true);
assert.deepEqual(evaluate("(+ 2 2)"), { failed: false, text: "4" });
const symbols = evaluate("(symbols)");
assert.equal(symbols.failed, false);
const names = symbols.text.slice(1, -1).split(" ");
assert.deepEqual(names, [...new Set(names)].sort());
for (const name of ["+", "factorial", "forever", "symbols"]) assert.ok(names.includes(name));
assert.ok(!names.includes("x"));
assert.deepEqual(evaluate("(let ((local 1)) (symbols))"), symbols);
assert.deepEqual(evaluate("(apply symbols '())"), symbols);
assert.deepEqual(evaluate("(symbols 1)"), { failed: true, text: "symbols expects 0 argument(s), got 1\n  at <input>:1:1 in symbols" });
const forms = [
  ["(let ((x 1)) (set! x 4) x)", "4"],
  ["(let* ((x 1) (y (+ x 2))) y)", "3"],
  ["(letrec ((even (lambda (n) (if (= n 0) #t (odd (- n 1))))) (odd (lambda (n) (if (= n 0) #f (even (- n 1)))))) (even 8))", "#t"],
  ["(let loop ((n 5) (sum 0)) (if (= n 0) sum (loop (- n 1) (+ sum n))))", "15"],
  ["(cond (#f missing) ((list 1 2) => length) (else missing))", "2"],
  ["(case 'loop ((other) missing) ((loop) 'yes))", "yes"],
  ["(when #t 1 2)", "2"],
  ["(unless #f 3 4)", "4"],
  ["`(a ,(+ 1 2) ,@'(4 5))", "(a 3 4 5)"],
  ["(let ((x 10)) `(outer `(inner ,x ,,x)))", "(outer (quasiquote (inner (unquote x) (unquote 10))))"],
  ["(do ((i 0 (+ i 1)) (sum 0 (+ sum i))) ((= i 5) sum))", "10"],
];
for (const [source, text] of forms) {
  assert.deepEqual(evaluate(source), { failed: false, text }, source);
}
for (const source of ["(letrec ((x x)) x)", "(set! absent 1)", ",x", "`(,@1)", "(cond (else))", "(do () ())"]) {
  assert.equal(evaluate(source).failed, true, source);
}
assert.match(evaluate("(do () (#f))").text, /^Evaluation limit reached/);
assert.deepEqual(evaluate("(+ 2 2)"), { failed: false, text: "4" });

assert.deepEqual(evaluate("(let loop ((n 3000)) (if (= n 0) n (loop (- n 1))))"), { failed: false, text: "0" });
assert.deepEqual(evaluate('(list (equal? \'(1 "x") \'(1 "x")) (substring "aλ😀z" 1 3) (string-length "λ😀"))'),
  { failed: false, text: '(#t "λ😀" 2)' });
assert.match(evaluate('(error "invalid" 42)').text, /^invalid: 42\n.*in error/);

function namedRequest(name, source, register = false) {
  const filename = new TextEncoder().encode(name);
  const namePointer = runtime.source_name_buffer(filename.length);
  new Uint8Array(runtime.memory.buffer, namePointer, filename.length).set(filename);
  const bytes = new TextEncoder().encode(source);
  const pointer = runtime.input_buffer(bytes.length);
  new Uint8Array(runtime.memory.buffer, pointer, bytes.length).set(bytes);
  const failed = register ? runtime.register_source() : runtime.evaluate_named();
  const result = JSON.parse(new TextDecoder().decode(new Uint8Array(runtime.memory.buffer, runtime.output_pointer(), runtime.output_length())));
  assert.equal(failed !== 0, result.error !== null);
  return result;
}
assert.equal(runtime.source_limit(), 128);
assert.equal(runtime.source_name_buffer(1025), 0);
assert.deepEqual(namedRequest("math.loop", "(module math (export square) (define (square n) (* n n)))", true),
  { evaluations: [], error: null });
assert.deepEqual(namedRequest("main.loop", '(load "math.loop") (import math square) (square 12)'), {
  evaluations: [{ value: "math", output: "" }, { value: "()", output: "" }, { value: "144", output: "" }],
  error: null,
});
assert.match(namedRequest("bad.loop", "\n(/ 1 0)").error, /bad.loop:2:1 in \//);
assert.match(namedRequest("main.loop", '(load "not-opened.loop")').error, /Source not registered/);
assert.match(namedRequest("bad\nname", "1", true).error, /control characters/);
for (let i = 0; i < 50; i++) evaluate("(letrec ((f (lambda () f))) 'done)");
const warmedMemory = runtime.memory.buffer.byteLength;
for (let i = 0; i < 500; i++) {
  assert.deepEqual(evaluate("(letrec ((f (lambda () f))) 'done)"), { failed: false, text: "done" });
}
assert.ok(runtime.memory.buffer.byteLength <= warmedMemory + 65_536, "unreachable closure cycles must not accumulate");
console.log("WASM integration checks passed.");

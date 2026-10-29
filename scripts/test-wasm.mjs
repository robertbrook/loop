import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const { instance } = await WebAssembly.instantiate(await readFile(new URL("../dist/loop.wasm", import.meta.url)));
const runtime = instance.exports;
function evaluate(source) {
  const bytes = new TextEncoder().encode(source);
  const pointer = runtime.input_buffer(bytes.length);
  new Uint8Array(runtime.memory.buffer, pointer, bytes.length).set(bytes);
  const failed = runtime.evaluate() !== 0;
  const text = new TextDecoder().decode(new Uint8Array(runtime.memory.buffer, runtime.output_pointer(), runtime.output_length()));
  return { failed, text };
}

assert.deepEqual(evaluate("(+ 1 2 3)"), { failed: false, text: "6" });
assert.deepEqual(evaluate("(define (factorial n) (if (<= n 1) 1 (* n (factorial (- n 1))))) (factorial 10)"), { failed: false, text: "factorial\n3628800" });
assert.deepEqual(evaluate("(factorial 5)"), { failed: false, text: "120" });
assert.deepEqual(evaluate("(map (lambda (x) (* x x)) '(1 2 3))"), { failed: false, text: "(1 4 9)" });
assert.deepEqual(evaluate("(let ((x 4)) (+ x 2))"), { failed: false, text: "6" });
assert.deepEqual(evaluate("(/ 1 0)"), { failed: true, text: "Division by zero" });
assert.deepEqual(evaluate("(+ 8 9)"), { failed: false, text: "17" });
assert.equal(evaluate("(define (forever) (forever)) (forever)").failed, true);
assert.deepEqual(evaluate("(+ 2 2)"), { failed: false, text: "4" });
console.log("WASM integration checks passed.");

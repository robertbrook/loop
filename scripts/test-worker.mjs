import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { builtAsset } from "./built-assets.mjs";

const bytes = await readFile(builtAsset("loop.wasm"));
const messages = [];
let ready;
const initialized = new Promise((resolve) => { ready = resolve; });
globalThis.self = {
  postMessage(message) {
    messages.push(message);
    if (message.type === "ready") ready();
    if (message.type === "load-error") throw new Error(message.text);
  },
};
globalThis.fetch = async (url) => {
  assert.ok(url.href.endsWith("/loop.wasm"), "only the runtime may be fetched");
  assert.equal(url.href, builtAsset("loop.wasm").href, "Worker and WASM must use the same build");
  return new Response(bytes);
};
await import(builtAsset("worker.js"));
await initialized;
assert.equal(messages[0].inputLimit, 65_536);
assert.equal(messages[0].sourceLimit, 128);
function execute(source, name = "main.loop", sources = []) {
  self.onmessage({ data: { type: "evaluate", source, name, sources } });
  const result = messages.at(-1);
  assert.equal(result.type, "result");
  return result;
}
const result = execute('(load "math.loop") (import math square) (square 9)', "main.loop", [
  ["math.loop", "(module math (export square) (define (square n) (* n n)))"],
]);
assert.equal(result.error, null);
assert.equal(result.evaluations.at(-1).value, "81");
assert.match(execute("\n(/ 1 0)", "failure.loop").error, /failure.loop:2:1 in \//);
assert.match(execute('(load "https://example.com/code")').error, /Source not registered/);
assert.match(execute(" ".repeat(65_537)).error, /input limit/);
assert.match(execute("1", "x".repeat(1025)).error, /filename/);
assert.match(execute("1", "main.loop", [["bad\nname", "1"]]).error, /control characters/);
assert.equal(execute("(+ 1 2)").evaluations[0].value, "3");
console.log("Worker file registration, diagnostics and WASM execution checks passed.");

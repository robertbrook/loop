import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { builtAsset } from "./built-assets.mjs";
import { browserHost } from "../web/host.js";

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
assert.ok(messages[0].symbols.includes("string-ref"));
assert.ok(!messages[0].symbols.includes("square"));
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
assert.ok(result.symbols.includes("square"), "imports update completion symbols");
assert.ok(!result.symbols.includes("n"), "local parameters are not globals");
assert.deepEqual(result.registeredSources, ["math.loop"]);
assert.deepEqual(execute("(square 10)").registeredSources, []);
assert.equal(execute("(square 10)").evaluations[0].value, "100");
assert.equal(execute('(load "values.loop") answer', "main.loop", [["values.loop", "(define answer 1)"]])
  .evaluations.at(-1).value, "1");
assert.equal(execute('(load "values.loop") answer', "main.loop", [["values.loop", "(define answer 2)"]])
  .evaluations.at(-1).value, "2");
const partial = execute("missing", "main.loop", [
  ["retained.loop", "(define retained 7)"], ["bad\nname", "1"],
]);
assert.deepEqual(partial.registeredSources, ["retained.loop"]);
assert.match(partial.error, /control characters/);
assert.equal(execute('(load "retained.loop") retained').evaluations.at(-1).value, "7");
const languageError = execute("missing", "main.loop", [["valid.loop", "(define valid 9)"]]);
assert.deepEqual(languageError.registeredSources, ["valid.loop"]);
assert.match(languageError.error, /Unknown symbol/);
assert.equal(execute('(load "valid.loop") valid').evaluations.at(-1).value, "9");
assert.match(execute("\n(/ 1 0)", "failure.loop").error, /failure.loop:2:1 in \//);
assert.match(execute('(load "https://example.com/code")').error, /Source not registered/);
assert.match(execute(" ".repeat(65_537)).error, /input limit/);
assert.match(execute("1", "x".repeat(1025)).error, /filename/);
assert.match(execute("1", "main.loop", [["bad\nname", "1"]]).error, /control characters/);
assert.equal(execute("(+ 1 2)").evaluations[0].value, "3");
const definedBeforeError = execute("(define completion-test 1) missing");
assert.ok(definedBeforeError.symbols.includes("completion-test"), "errors can leave definitions behind");
const shadowed = execute("(define symbols 12) (define λ-name 7)");
assert.ok(shadowed.symbols.includes("λ-name"), "completion must not call the shadowable symbols builtin");
assert.equal(execute("symbols").evaluations[0].value, "12", "completion must not mutate bindings");
assert.equal(execute('(write-file "created.loop" "(define created 42)") (load "created.loop") created')
  .evaluations.at(-1).value, "42");
assert.ok(messages.some(message => message.type === "file-written" && message.name === "created.loop"));
assert.equal(execute('(read-file "created.loop")').evaluations[0].value, '"(define created 42)"');
assert.equal(execute('(write-file "values.loop" "(define answer 99)") (load "values.loop") answer')
  .evaluations.at(-1).value, "99", "writes must update previously registered load sources");
assert.equal(execute('(json-get (json-parse "{\\"x\\":3}") "x")').evaluations[0].value, "3");
assert.equal(execute('(node-text (html-parse "<p>hello<br>world"))').evaluations[0].value, '"helloworld"');
assert.equal(execute('(defmacro twice (x) `(+ ,x ,x)) (twice (eval \'(+ 1 2)))')
  .evaluations.at(-1).value, "6");
assert.ok(execute("(random)").symbols.includes("random-integer"));

const requests = [];
let responseStatus = 404, responseBody = "not found", throwNetwork = false;
globalThis.XMLHttpRequest = class {
  headers = [];
  open(method, url, async) { requests.push(this); this.method = method; this.url = url; assert.equal(async, false); }
  setRequestHeader(name, value) { this.headers.push([name, value]); }
  send(body) {
    if (throwNetwork) throw new Error("Network unavailable");
    this.body = body; this.status = responseStatus; this.responseText = responseBody;
  }
  getAllResponseHeaders() { return "content-type: text/plain\r\nx-test: yes\r\n"; }
};
assert.equal(execute('(http-status (http-get "https://example.test/missing"))').evaluations[0].value, "404");
responseStatus = 201;
responseBody = '{"ok":true}';
assert.equal(execute('(json-get (json-parse (http-body (http-request "POST" "https://example.test/" \'(("Content-Type" "application/json")) "{\\"a\\":1}"))) "ok")')
  .evaluations[0].value, "#t");
assert.equal(requests.at(-1).method, "POST");
assert.equal(requests.at(-1).body, '{"a":1}');
assert.deepEqual(requests.at(-1).headers, [["Content-Type", "application/json"]]);
assert.match(execute('(http-request "GET" "https://example.test/" \'(("Cookie" "private")) "")').error, /does not allow/);
throwNetwork = true;
assert.match(execute('(http-get "https://example.test/")').error, /connectivity, CORS/);
throwNetwork = false;
responseBody = "x".repeat(65_537);
assert.match(execute('(http-get "https://example.test/")').error, /size limit/);
const fullFiles = new Map(Array.from({ length: 128 }, (_, i) => [`${i}`, ""]));
const fullHost = browserHost(fullFiles, () => {});
assert.throws(() => fullHost({ op: "write_file", path: "extra", contents: "" }), /128/);
assert.throws(() => fullHost({ op: "read_file", path: "missing" }), /not registered/);
console.log("Worker file registration, diagnostics and WASM execution checks passed.");

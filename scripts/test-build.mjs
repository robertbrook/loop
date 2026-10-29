import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { assetDirectory, builtAsset, html, indexURL } from "./built-assets.mjs";

const sourceDirectory = new URL("../web/", import.meta.url);
const files = (await readdir(sourceDirectory)).sort();
const wasmPath = "target/wasm32-unknown-unknown/release/loop_lisp.wasm";
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
let checksums = "";
for (const name of files) {
  const source = await readFile(new URL(name, sourceDirectory));
  assert.deepEqual(await readFile(builtAsset(name)), source, `Built ${name} must match its source`);
  checksums += `${hash(source)}  web/${name}\n`;
}
const wasm = await readFile(new URL(`../${wasmPath}`, import.meta.url));
assert.deepEqual(await readFile(builtAsset("loop.wasm")), wasm);
checksums += `${hash(wasm)}  ${wasmPath}\n`;
const revision = hash(checksums).slice(0, 16);
assert.ok(assetDirectory.pathname.endsWith(`/assets/${revision}/`),
  "Every HTML, CSS, JS and WASM change must invalidate the entire asset graph");

const entrypoints = [...html.matchAll(/(?:src|href)="(\.\/[^"]+)"/g)].map((match) => match[1]);
assert.equal(entrypoints.length, 3);
for (const entrypoint of entrypoints) {
  const resolved = new URL(entrypoint, indexURL);
  assert.equal(new URL("./", resolved).href, assetDirectory.href, "All entrypoints must use one build");
  await readFile(resolved);
}
assert.ok(html.indexOf("/theme.js") < html.indexOf("/style.css"), "Saved theme must apply before styles");
for (const name of files.filter((name) => name.endsWith(".js"))) {
  const script = await readFile(builtAsset(name), "utf8");
  for (const match of script.matchAll(/(?:from\s+|new URL\()["'](\.\/[^"']+)["']/g)) {
    const dependency = new URL(match[1], builtAsset(name));
    assert.equal(new URL("./", dependency).href, assetDirectory.href,
      `${name} dependency must stay in the same versioned directory`);
    await readFile(dependency);
  }
}
const sourceHTML = await readFile(new URL("index.html", sourceDirectory), "utf8");
assert.equal(html, sourceHTML.replaceAll(/((?:src|href)=")\.\//g, `$1./assets/${revision}/`));
console.log("Versioned HTML, CSS, module, worker and WASM asset graph checks passed.");

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { runInNewContext } from "node:vm";

const source = await readFile(new URL("../web/theme.js", import.meta.url), "utf8");
function setup(saved = null, readError, writeError) {
  const listeners = {};
  const warnings = [];
  const writes = [];
  const select = {
    value: "",
    addEventListener(type, listener) { listeners[type] = listener; },
  };
  const root = { dataset: {} };
  let loaded;
  runInNewContext(source, {
    document: {
      documentElement: root,
      getElementById: (id) => { assert.equal(id, "theme"); return select; },
      addEventListener: (type, listener) => {
        assert.equal(type, "DOMContentLoaded");
        loaded = listener;
      },
    },
    localStorage: {
      getItem: (key) => {
        assert.equal(key, "loop-theme");
        if (readError) throw readError;
        return saved;
      },
      setItem: (key, value) => {
        if (writeError) throw writeError;
        writes.push([key, value]);
      },
    },
    console: { warn: (...args) => warnings.push(args) },
  });
  return {
    root, select, warnings, writes, loaded,
    change(value) { select.value = value; listeners.change(); },
  };
}
for (const saved of [null, "system", "light", "dark", "invalid"]) {
  const theme = setup(saved);
  const expected = ["light", "dark"].includes(saved) ? saved : "system";
  assert.equal(theme.root.dataset.theme, expected, "Theme must apply before DOMContentLoaded");
  theme.loaded();
  assert.equal(theme.select.value, expected);
  for (const value of ["dark", "light", "system"]) {
    theme.change(value);
    assert.equal(theme.root.dataset.theme, value);
    assert.deepEqual(theme.writes.at(-1), ["loop-theme", value]);
  }
}
const denied = setup(null, new DOMException("Blocked", "SecurityError"));
assert.equal(denied.root.dataset.theme, "system");
assert.equal(denied.warnings.length, 1);
denied.loaded();
denied.change("dark");
assert.equal(denied.root.dataset.theme, "dark");
for (const name of ["SecurityError", "QuotaExceededError"]) {
  const theme = setup("dark", undefined, new DOMException("Blocked", name));
  theme.loaded();
  theme.change("light");
  assert.equal(theme.root.dataset.theme, "light");
  assert.equal(theme.warnings.length, 1);
}
assert.throws(() => setup(null, new Error("Unexpected storage failure")), /Unexpected storage failure/);
const unexpectedWrite = setup(null, undefined, new Error("Unexpected write failure"));
unexpectedWrite.loaded();
assert.throws(() => unexpectedWrite.change("dark"), /Unexpected write failure/);
console.log("Theme selection, persistence, and storage-error checks passed.");

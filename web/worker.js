import { browserHost, hostImports } from "./host.js";

let runtime;
const files = new Map();
const encoder = new TextEncoder();
const decoder = new TextDecoder();

async function initialize() {
  const response = await fetch(new URL("./loop.wasm", import.meta.url));
  if (!response.ok) throw new Error(`Could not load WebAssembly (${response.status}). Reload to retry.`);
  const handle = browserHost(files, (name, source) => self.postMessage({ type: "file-written", name, source }));
  const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), hostImports(() => runtime.memory, handle));
  runtime = instance.exports;
  self.postMessage({ type: "ready", inputLimit: runtime.input_limit(), sourceLimit: runtime.source_limit(), symbols: symbols() });
}

function readResponse() {
  return JSON.parse(decoder.decode(new Uint8Array(runtime.memory.buffer, runtime.output_pointer(), runtime.output_length())));
}

function symbols() {
  runtime.completion_symbols();
  return readResponse();
}

function request(operation, source, name) {
  const bytes = encoder.encode(source);
  if (bytes.length > runtime.input_limit()) {
    return { evaluations: [], error: `${name}: program exceeds the 64 KiB input limit.` };
  }
  const filename = encoder.encode(name);
  const namePointer = runtime.source_name_buffer(filename.length);
  if (!namePointer) return { evaluations: [], error: "Source filename exceeds the 1024-byte limit." };
  new Uint8Array(runtime.memory.buffer, namePointer, filename.length).set(filename);
  const pointer = runtime.input_buffer(bytes.length);
  new Uint8Array(runtime.memory.buffer, pointer, bytes.length).set(bytes);
  runtime[operation]();
  return readResponse();
}

self.onmessage = ({ data }) => {
  if (data.type !== "evaluate") return;
  try {
    if (!runtime) throw new Error("The runtime is not ready yet.");
    const start = performance.now();
    const registeredSources = [];
    for (const [name, source] of data.sources) {
      const registered = request("register_source", source, name);
      if (registered.error !== null) {
        self.postMessage({ type: "result", ...registered, registeredSources, symbols: symbols(), elapsed: performance.now() - start });
        return;
      }
      registeredSources.push(name);
      files.set(name, source);
    }
    const result = request("evaluate_named", data.source, data.name);
    self.postMessage({ type: "result", ...result, registeredSources, symbols: symbols(), elapsed: performance.now() - start });
  } catch (error) {
    // A WASM trap can leave the interpreter unusable; the UI replaces the worker.
    self.postMessage({ type: "fatal", text: error.message });
  }
};

initialize().catch((error) => self.postMessage({ type: "load-error", text: error.message }));

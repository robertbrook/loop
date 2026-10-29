let runtime;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

async function initialize() {
  const response = await fetch(new URL("./loop.wasm", import.meta.url));
  if (!response.ok) throw new Error(`Could not load WebAssembly (${response.status}). Reload to retry.`);
  const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {});
  runtime = instance.exports;
  self.postMessage({ type: "ready", inputLimit: runtime.input_limit(), sourceLimit: runtime.source_limit() });
}

function request(source, name, register) {
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
  if (register) runtime.register_source();
  else runtime.evaluate_named();
  return JSON.parse(decoder.decode(new Uint8Array(runtime.memory.buffer, runtime.output_pointer(), runtime.output_length())));
}

self.onmessage = ({ data }) => {
  if (data.type !== "evaluate") return;
  try {
    if (!runtime) throw new Error("The runtime is not ready yet.");
    const start = performance.now();
    for (const [name, source] of data.sources) {
      const registered = request(source, name, true);
      if (registered.error !== null) {
        self.postMessage({ type: "result", ...registered, elapsed: performance.now() - start });
        return;
      }
    }
    const result = request(data.source, data.name, false);
    self.postMessage({ type: "result", ...result, elapsed: performance.now() - start });
  } catch (error) {
    // A WASM trap can leave the interpreter unusable; the UI replaces the worker.
    self.postMessage({ type: "fatal", text: error.message });
  }
};

initialize().catch((error) => self.postMessage({ type: "load-error", text: error.message }));

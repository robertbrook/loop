let runtime;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

async function initialize() {
  const response = await fetch(new URL("./loop.wasm", import.meta.url));
  if (!response.ok) throw new Error(`Could not load WebAssembly (${response.status}). Reload to retry.`);
  const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {});
  runtime = instance.exports;
  self.postMessage({ type: "ready" });
}

self.onmessage = ({ data }) => {
  if (data.type !== "evaluate") return;
  try {
    if (!runtime) throw new Error("The runtime is not ready yet.");
    const bytes = encoder.encode(data.source);
    if (bytes.length > 65536) throw new Error("Program exceeds the 64 KiB input limit.");
    const pointer = runtime.input_buffer(bytes.length);
    new Uint8Array(runtime.memory.buffer, pointer, bytes.length).set(bytes);
    const start = performance.now();
    const failed = runtime.evaluate() !== 0;
    const text = decoder.decode(new Uint8Array(runtime.memory.buffer, runtime.output_pointer(), runtime.output_length()));
    self.postMessage({ type: "result", text, failed, elapsed: performance.now() - start });
  } catch (error) {
    // A WASM trap can leave the interpreter unusable; the UI replaces the worker.
    self.postMessage({ type: "fatal", text: error.message });
  }
};

initialize().catch((error) => self.postMessage({ type: "load-error", text: error.message }));

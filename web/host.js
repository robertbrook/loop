export function hostImports(memory, handle) {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let response = new Uint8Array();
  return {
    env: {
      loop_random_seed() {
        const values = new Uint32Array(1);
        globalThis.crypto.getRandomValues(values);
        return values[0];
      },
      loop_host_request(pointer, length) {
        try {
          const request = JSON.parse(decoder.decode(new Uint8Array(memory().buffer, pointer, length)));
          response = encoder.encode(JSON.stringify({ response: handle(request), error: null }));
        } catch (error) {
          response = encoder.encode(JSON.stringify({ response: null, error: error.message }));
        }
        return response.length;
      },
      loop_host_response(pointer, length) {
        if (length !== response.length) return 0;
        new Uint8Array(memory().buffer, pointer, length).set(response);
        return length;
      },
    },
  };
}

export function browserHost(files, written, xhrFactory = () => new XMLHttpRequest()) {
  const encoder = new TextEncoder();
  const limit = 65_536;
  return (request) => {
    const response = { status: null, headers: [], body: "" };
    if (request.op === "read_file") {
      if (!files.has(request.path)) throw new Error(`Source not registered: ${request.path}. Open the file before reading or loading it.`);
      response.body = files.get(request.path);
    } else if (request.op === "write_file") {
      if (!files.has(request.path) && files.size >= 128) throw new Error("Open at most 128 files per session.");
      if (encoder.encode(request.contents).length > limit) throw new Error("File size limit reached (64 KiB).");
      files.set(request.path, request.contents);
      written(request.path, request.contents);
    } else if (request.op === "http") {
      const xhr = xhrFactory();
      xhr.open(request.method, request.url, false);
      xhr.timeout = 2500;
      let oversized = false;
      xhr.onprogress = event => {
        if (event.loaded > limit) { oversized = true; xhr.abort(); }
      };
      for (const [name, value] of request.headers) {
        const lower = name.toLowerCase();
        if (lower.startsWith("proxy-") || lower.startsWith("sec-") || [
          "accept-charset", "accept-encoding", "access-control-request-headers",
          "access-control-request-method", "connection", "content-length", "cookie",
          "cookie2", "date", "dnt", "expect", "host", "keep-alive", "origin",
          "referer", "set-cookie", "te", "trailer", "transfer-encoding", "upgrade", "via",
        ].includes(lower)) throw new Error(`The browser does not allow setting the ${name} header.`);
        xhr.setRequestHeader(name, value);
      }
      try {
        xhr.send(request.body || null);
      } catch (error) {
        throw new Error(oversized ? "HTTP response size limit reached (64 KiB)." : `HTTP failed: ${error.message}. Check connectivity, CORS, and the URL.`);
      }
      if (!xhr.status) throw new Error("HTTP failed: no response status. Check connectivity and CORS.");
      response.status = xhr.status;
      response.body = xhr.responseText;
      if (oversized || encoder.encode(response.body).length > limit) throw new Error("HTTP response size limit reached (64 KiB).");
      response.headers = xhr.getAllResponseHeaders().trim().split(/[\r\n]+/).filter(Boolean).map(line => {
        const colon = line.indexOf(":");
        if (colon < 1) throw new Error("Invalid HTTP response header.");
        return [line.slice(0, colon).trim().toLowerCase(), line.slice(colon + 1).trim()];
      });
    } else {
      throw new Error(`Unsupported host operation: ${request.op}`);
    }
    return response;
  };
}

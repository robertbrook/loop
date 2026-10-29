# Loop

Loop is a Lisp dialect implemented in Rust, with a WebAssembly browser
workbench. The language is the project; the workbench is one interface to it,
not a demo-only or education-only boundary. Loop takes inspiration from
Scheme, but develops its own dialect rather than targeting Scheme compatibility.
Deployment target: https://robertbrook.github.io/loop/.

## Develop

Install Rust with rustup, then:

```sh
rustup target add wasm32-unknown-unknown
cargo test
bash scripts/build.sh
node scripts/test-wasm.mjs
python3 -m http.server 8000 --directory dist
```

Open http://localhost:8000. Serve over HTTP rather than opening the HTML file
directly: the application loads a WASM module and a module worker. There are no
JavaScript or Rust package dependencies.

## Language

Numbers, booleans (`#t`, `#f`), strings, symbols, and proper lists; lexical
closures; recursive definitions; `define`, `lambda`, `if`, `quote`, `begin`,
parallel `let`, `and`, and `or`. Arithmetic, comparisons, list operations,
`map`, `filter`, `apply`, and output procedures are available. The site's
expandable reference lists every builtin and the example programs demonstrate
the current dialect. Macros, mutation, dotted pairs, tail-call optimization,
and a tracing garbage collector are not implemented yet.

Each top-level expression prints its return value. `display` writes unquoted
text, and both `display` and `newline` return `()`. Definitions persist between
program and REPL evaluations until **Reset session**. Up/down arrows recall
REPL history. Ctrl/Cmd+Enter runs the editor.

Programs are limited to 64 KiB, 100,000 evaluation steps, 128 nested calls,
10,000 elements in constructed lists, and 128 KiB of output. A worker keeps
evaluation off the UI thread, and the browser restarts it after a 3-second
timeout or WASM trap. Rust uses reference-counted environments rather than a
tracing garbage collector: recursive closures may retain memory until the
worker is reset. Reset long-running sessions to release it.

## Deployment

`.github/workflows/pages.yml` tests the interpreter, builds the WASM module and
static site, and deploys `dist/` through GitHub Pages Actions. The `main` and
initial implementation branch trigger deployment. Pages must use **GitHub
Actions** as its source. All assets use relative URLs for the `/loop/` base path.
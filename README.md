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
node scripts/test-build.mjs
node scripts/test-wasm.mjs
node scripts/test-worker.mjs
node scripts/test-browser.mjs
node scripts/test-theme.mjs
python3 -m http.server 8000 --directory dist
```

Open http://localhost:8000. Serve over HTTP rather than opening the HTML file
directly: the application loads a WASM module and a module worker. No JavaScript
package installation is needed. Rust dependencies provide JSON, XML, HTML5
parsing and native HTTPS; versions are pinned in `Cargo.lock`.

The only stylesheet is the vendored, unmodified Bootstrap **6.0.0-alpha.1**
development snapshot from
[`twbs/bootstrap` at `c967e2c`](https://github.com/twbs/bootstrap/tree/c967e2cbe9c30d51348fe8d42d34b84c3e7f555c).
This is not a published Bootstrap release. Its MIT license and CSS source map
are included beside the stylesheet. Components, layout, states, and themes use
Bootstrap classes and `data-bs-theme`; there are no custom stylesheets or
inline styles. System mode tracks changes to the operating system theme.

The browser REPL suggests special forms and live global bindings as you type
a symbol, including definitions and imports from the current session.
Use Up/Down to choose, Tab or Enter to complete, Escape to dismiss, or click
a suggestion. Completion replaces only the symbol at the caret and stays off
inside strings and comments. With suggestions closed, Up/Down recall history.
Resetting the session also resets its binding suggestions.

### Native CLI

The same interpreter also runs outside the browser:

```sh
cargo run -- -e '(+ 1 2)'
cargo run -- examples/main.loop
printf '(+ 20 22)\n' | cargo run --quiet
cargo run                 # multiline terminal REPL
cargo install --path .    # install the loop command
```

`loop FILE ...` runs files left to right in one persistent session. `-` reads
standard input; `-e PROGRAM` evaluates one program. Errors go to stderr with
a nonzero exit status in batch mode. The terminal REPL supports multiline
forms and recovers after language errors. Use `loop --help` for usage.

## Language

Numbers, booleans (`#t`, `#f`), strings, symbols, and proper lists; lexical
closures; recursive definitions; assignment; conditional, binding, iteration,
and quotation forms. Arithmetic, comparisons, list operations,
`map`, `filter`, `apply`, and output procedures are available. The
reference below lists every builtin and the example programs demonstrate
the current dialect. Proper tail calls, automatic collection of unreachable
closure cycles, named modules, and host-controlled source loading are supported.
Lisp-style macros, uninterned generated symbols, and evaluation of quoted
data are supported. Mutable pairs and dotted pairs are not implemented.

| Category | Names |
| --- | --- |
| Definitions and assignment | `define lambda set!` |
| Conditionals and sequencing | `if begin and or cond case when unless` |
| Bindings and iteration | `let` (including named let), `let* letrec do` |
| Quotation | `quote quasiquote unquote unquote-splicing` |
| Modules and source loading | `module import load` |
| Numbers | `+ - * / = < > <= >= modulo abs` |
| Lists | `list car cdr cons append length map filter apply` |
| Equality | `equal? eq?` |
| Predicates | `not null? number? list? boolean? string? symbol? procedure?` |
| Strings | `string-length string-append substring string-ref string=?` |
| Conversion | `string->symbol symbol->string number->string string->number` |
| Output, errors, and introspection | `display newline error symbols` |
| Macros and evaluation | `defmacro gensym eval` |
| Random numbers | `random random-integer random-seed!` |
| Text files | `read-file write-file` |
| HTTP | `http-request http-get http-head http-delete http-options http-post http-put http-patch http-status http-headers http-body` |
| JSON | `json-parse json-stringify json-get json-set json-remove` |
| XML and HTML | `xml-parse xml-stringify html-parse html-stringify` |
| Markup trees | `node-tag node-attributes node-children node-text node-find node-attribute node-with-attribute node-with-children` |

### Macros, quotation, and eval

`quote` (or `'`) returns data without executing it. `quasiquote` (backtick)
builds data with `unquote` (comma) and list-only `unquote-splicing` (`,@`).
These forms work with macros and `eval` without a separate syntax language:

```lisp
(defmacro twice (expression)
  `(+ ,expression ,expression))
(twice 21) ; 42

(defmacro once (expression &rest body)
  (let ((temporary (gensym)))
    `(let ((,temporary ,expression))
       ,@body
       ,temporary)))

(eval '(+ 20 22)) ; 42
```

`defmacro` accepts a name, parameter list, and one or more body expressions.
The optional `&rest name` must be the final parameter pair. Arguments arrive
as unevaluated data; the transformer's result is evaluated in the caller's
lexical scope. Transformer bodies retain their definition scope. Macros
can be local, aliased, and explicitly exported/imported by modules, but are
not procedures and cannot be passed to `apply` or `map`. Expansion happens at
evaluation time, shares execution limits, and preserves call-site diagnostics.
There is no automatic hygiene: use `gensym` for introduced binding names.

`gensym` takes no arguments and returns a fresh uninterned symbol. Cloning,
quoting, and evaluating data preserve its identity; converting its name to a
string and back does not. Its printed `#:g...` notation is diagnostic, not a
readable encoding of that identity.

`eval` is a first-class procedure taking one datum. It evaluates in the
session's **global** environment, not the caller's local scope. Numbers,
booleans, strings, symbols, and lists are executable data; embedded procedure
or macro values are rejected. Generated-code errors include `<eval>` locations.
Tail evaluation and generated syntax remain subject to the normal step,
depth, and list limits.

### Random numbers

`(random)` produces a pseudo-random number in `[0, 1)`.
`(random-integer bound)` produces an integer from zero up to, but excluding,
`bound`; rejection sampling avoids modulo bias. Bounds must be integers from
1 through 2147483646. `(random-seed! seed)` accepts the same integer range,
resets the sequence, and returns `()`.

These are ordinary Lisp definitions using a private Park–Miller generator.
Each session starts with an operating-system/browser cryptographic random seed;
use `random-seed!` for a reproducible sequence. Library definitions share that
state, including in modules.
This generator is **not cryptographically secure**.

### Text files and HTTP

```lisp
(write-file "answer.json" (json-stringify '(object ("answer" 42))))
(json-get (json-parse (read-file "answer.json")) "answer") ; 42

(define response
  (http-request "POST" "https://example.com/api"
                '(("Content-Type" "application/json"))
                (json-stringify '(object ("message" "hello")))))
(http-status response)
(http-headers response)
(http-body response)
```

`read-file` reads UTF-8 text. `write-file` replaces the file's contents and
returns `()`. In the CLI, paths are relative to the **working directory**
(unlike source-relative `load`), and normal filesystem permissions apply.
The CLI executes trusted programs: these operations can write files and
contact the network.

In the browser, Open files grants access to the selected text files by
filename. Writes create or replace files in the browser workspace; they do
not silently modify files on disk. Select the resulting file and use
**Save file** to download it. Opened and written files survive runtime resets
in the page but not reloads. Files written as Lisp sources can be loaded in
the same evaluation. Newer in-flight editor changes are not overwritten.
File contents and HTTP request/response bodies are limited to 64 KiB.

`http-request` takes an uppercase method, HTTP(S) URL, a list of string header
pairs, and a text body. Methods are GET, POST, PUT, PATCH, DELETE, HEAD, and
OPTIONS. GET and HEAD require an empty body. `http-get`, `http-head`, and
`http-delete` and `http-options` take a URL; `http-post`, `http-put`, and `http-patch` take a URL
and body. Use the generic procedure to supply headers.

Responses are `(response status headers body)` data, including non-2xx
statuses; network, TLS, timeout, and size errors raise diagnostics. Native
requests follow at most five redirects and time out after three seconds.
Browser requests run synchronously in the worker, use a 2.5-second network
timeout, and remain subject to the page's execution deadline, CORS, mixed
content, and forbidden-header rules. They never bypass browser protections.
URLs are limited to 8192 bytes and headers to 128 pairs / 64 KiB per direction.
Native HTTP uses Rustls with public certificate roots.

Library embedders opt in through `Interpreter::set_host_handler`; no native
file or network access exists in a bare interpreter until a handler is supplied.
The WASM host uses explicit request/response imports, independent of source
loading. Source files are still never fetched automatically over HTTP.

### JSON, XML, and HTML data

`json-parse` reads text; `json-stringify` writes it. The data representation is
explicit, including empty containers:

```lisp
'(object ("name" "Loop") ("values" (array 1 #t null)))
'(object) ; {}
'(array)  ; []
'null     ; null
```

Strings, booleans, and finite numbers use ordinary Lisp values. Numbers use
Loop's existing `f64` representation, including its precision limits.
`json-get` looks up an object string key or zero-based array index.
`json-set` returns a new value with an object key added/replaced or an existing
array item replaced. `json-remove` returns a new value with a key/item removed.
Missing keys, invalid indexes, malformed tagged data, and duplicate object
keys supplied to the writer raise errors; values are not mutated.

XML and HTML share tagged trees:

```lisp
'(document
   (element "p" (("id" "greeting"))
     "Hello "
     (element "strong" () "world")))
```

Element children follow the attribute list. Text is a string;
comments are `(comment "text")`, processing instructions are
`(instruction "target" "text")`, and HTML doctypes use
`(doctype "html" "" "")`.
`xml-parse` validates XML, decodes entities and CDATA, and rejects DTDs;
XML declarations and lexical formatting are not preserved. Namespace
declarations are retained for writing.
`html-parse` uses the HTML5 tree builder, repairs malformed HTML, and inserts
the normal `html`, `head`, and `body` nodes. Both serializers escape text and
attributes; HTML serialization also handles void and raw-text elements.
HTML doctype nodes retain their public and system identifiers when present.
Trees are data, never rendered into the workbench as HTML.

`node-find` returns matching elements (including the starting element) by tag
name; `node-text` joins descendant text. `node-tag`, `node-attributes`,
`node-children`, and `node-attribute` inspect an element.
`node-with-attribute` adds/replaces an attribute;
`node-with-children` replaces its children. Both return new trees.
Parsers and serializers use the normal execution/depth/list limits and a
128 KiB text bound; serialization normalizes formatting rather than preserving
source bytes.

Numbers are 64-bit floats. Only `#f` is false. Strings support `\n`, `\t`,
`\"`, and `\\` escapes; comments begin with `;`. Quoting (`'(1 2 3)`)
returns data without evaluating it. Functions use lexical scope; `let`
bindings are evaluated in the enclosing scope. `if` requires both branches.

### Special forms

| Syntax | Behavior |
| --- | --- |
| `(set! name value)` | Update the nearest existing lexical binding; return `()`. An unknown name is an error. |
| `(cond (test body ...) ... (else body ...))` | Evaluate the first true clause. A test-only clause returns its test value. `(test => recipient)` calls the recipient with that value. |
| `(case key ((datum ...) body ...) ... (else body ...))` | Evaluate the key once and match literal data without evaluating them. Loop compares numbers, booleans, symbols, strings, and proper lists by value. `((datum ...) => recipient)` and `(else => recipient)` pass the key to a procedure. |
| `(when test body ...)` / `(unless test body ...)` | Run the body when the test is true / false. |
| `(let ((name init) ...) body ...)` | Evaluate initializers in the outer scope, then introduce the bindings together. |
| `(let name ((parameter init) ...) body ...)` | Bind a local recursive procedure and call it with the initializer values. Initializers use the outer scope; the procedure name stays local. |
| `(let* ((name init) ...) body ...)` | Introduce bindings sequentially, each in a fresh scope. Later bindings may reuse names without changing earlier closures. |
| `(letrec ((name init) ...) body ...)` | All names are visible to all initializers and the body. Evaluate initializers left to right, then initialize all bindings together. Reading or assigning an uninitialized binding is an error. Supports mutually recursive closures. |
| `(do ((name init step) ...) (test result ...) body ...)` | Initialize in the outer scope. Test before each iteration, run the body if false, then compute steps left to right in the current scope and introduce their results simultaneously in fresh iteration bindings. Omitted steps retain their binding's value. Return the final result sequence when the test is true. |
| `` (quasiquote template) `` or `` `template `` | Return template data, evaluating `,expression` (`unquote`) and inserting elements of `,@expression` (`unquote-splicing`). Splicing requires a list and a surrounding list position. Nested quasiquotes preserve escapes until their matching level. |

`else` must be the final clause and have a body (or a recipient for `case`).
`cond` and `case` return `()` if no clause matches. `when` and `unless` return
`()` if skipped. Bodies and result sequences return their final expression,
or `()` when empty where allowed. Binding forms require a body; duplicate
binding names are rejected except in `let*`. `unquote` and `unquote-splicing`
outside quasiquote are errors. These forms are syntax, not first-class
procedures, and do not appear in `(symbols)`.

```scheme
(letrec ((even (lambda (n) (if (= n 0) #t (odd (- n 1)))))
         (odd  (lambda (n) (if (= n 0) #f (even (- n 1))))))
  (even 10))
; => #t

(do ((i 0 (+ i 1))
     (total 0 (+ total i)))
    ((= i 5) total))
; => 10

(let ((name 'loop) (items '(1 2 3)))
  `(language ,name values ,@items))
; => (language loop values 1 2 3)
```

### Modules and files

```scheme
; math.loop
(module math (export square)
  (define (square x) (* x x)))

; main.loop
(load "math.loop")
(import math square)
(square 12)
```

`module` evaluates its body in a fresh lexical environment containing the
standard library, not the caller's globals. Only explicitly exported local
bindings are available to `import`; other definitions stay private. A module
is registered only after its body and exports succeed. Module names are
unique for the session, and dependencies must be loaded before import.
`import` requires explicit binding names and rejects duplicate names, missing
exports, and overwriting existing local bindings before installing any imports.
Imported values are snapshots, not shared binding slots; imported closures
share the exporting module's private state.

`load` evaluates a source file in the current lexical environment and returns
its final value, or `()` for an empty file. Files are evaluated on every load,
not cached; loading a module twice therefore reports a duplicate module.
Nested loads share the caller's execution and output budgets, have a maximum
nesting of 32 files, and reject cycles.

The CLI resolves paths relative to the file containing the `load` expression,
including loads inside procedures called later. REPL and command-line loads
resolve relative to the working directory. File errors include their path.
In the browser, **Open files** registers selected files without executing them;
select a file in the editor, then Run. Open dependencies too, and use their
exact filenames in `load`. No source code is fetched from URLs. You can switch
between opened files without losing edits. Up to 128 files of 64 KiB each can
be registered, with filenames of at most 1024 UTF-8 bytes.

### Everyday procedures

`equal?` and `eq?` take two arguments. Both compare immutable data by value,
including nested lists and strings; procedures compare by identity, not body
text. These names do not promise Scheme's distinct equality semantics.
The type predicates take one argument and return a boolean.

String positions count Unicode scalar values, not UTF-8 bytes or visual
grapheme clusters. `(substring text start end)` uses a half-open range;
`(string-ref text index)` returns a one-scalar string. Indices must be
nonnegative integers in range. `string-length` counts scalars,
`string-append` concatenates zero or more strings, and `string=?` compares
two or more strings exactly. Constructed strings are limited to 128 KiB.

The conversion procedures each take one argument. `string->symbol` requires
one readable symbol name; `string->number` requires a finite number.
Invalid conversions report errors rather than returning a success-shaped
placeholder. `(error "message" value ...)` raises a diagnostic with optional
printed values. Reader and evaluator diagnostics include source names,
one-based line/column positions, and bounded Lisp call-site traces. Tail calls
replace frames rather than retaining unbounded history.

### Sessions and limits

Each top-level expression prints its return value. `display` writes unquoted
text, and both `display` and `newline` return `()`. Definitions persist between
program and REPL evaluations until **Reset session**. Up/down arrows recall
REPL history. Ctrl/Cmd+Enter runs the editor. Reset clears definitions and
modules, but keeps explicitly opened files available for loading again.

The header's theme selector offers **System**, **Light**, and **Dark**.
System follows your device preference automatically; an explicit choice
overrides it. Your choice is saved locally when browser storage is available.

`(symbols)` returns a sorted list of symbol values naming the current global
bindings, including builtins and user definitions. Local bindings and special
forms such as `define` and `if` are not included. It takes no arguments and can
be passed to higher-order procedures, for example `(apply symbols '())`.

Programs are limited to 64 KiB, 100,000 evaluation steps, 128 levels of non-tail
evaluation nesting,
10,000 elements in constructed lists, and 128 KiB of output. A worker keeps
evaluation off the UI thread, and the browser restarts it after a 3-second
timeout or WASM trap. Tail calls reuse the evaluator loop, including mutual
recursion, named `let`, conditional and sequence tails, and calls through
`apply`; they still consume evaluation steps.

Immutable values use Rust ownership and reference counting. A tracing
collector marks environments reachable from globals, module exports, and
current results, then severs unreachable environment/closure cycles after
every request, including failures. Dropping a native interpreter also releases
its cycles. Collection happens between requests, not during active evaluation;
live global definitions and modules remain until reset. WASM memory pages
retain their high-water allocation and are reused rather than shrinking.

## Implementation

The evaluator executes core forms directly. Ordinary `let` expands to a
lambda call, `let*` to nested lambda calls, and `when`/`unless` to `if` and
`begin`. Expansion introduces no temporary variable names and validates
bindings before evaluating initializers. Quasiquote, `letrec`, named `let`,
and `do` retain dedicated implementations. Expanded syntax counts toward
the evaluation and depth limits.

Binding and conditional-clause parsing use typed variants instead of string
flags. Resource limits are named constants in `src/lib.rs`; the WASM
`input_limit()` export supplies the browser's input bound. Browser-only
timeouts and result-history limits live in `web/limits.js`.

The reader builds an `Expr` tree with source locations, separate from runtime
`Value` data. Procedure bodies retain expressions; quoting converts expressions
to immutable data. Runtime values never carry syntax wrappers.

Rust errors are `Diagnostic` values with a message, an `ErrorKind`, and bounded
`TraceFrame` entries containing structured source locations. Incomplete reader
input has its own kind, so the terminal REPL does not inspect error wording.
Diagnostics are formatted only at CLI and WASM presentation boundaries.
The host source-loader callback can still return ordinary string errors.

One typed builtin registry owns each builtin procedure's name, argument-count rule,
and implementation handler. It supplies both global bindings and dispatch,
including tail calls through `apply`; aliases retain their existing semantics.

The public Rust `Interpreter` API provides `execute`, `execute_named`,
`register_source`, and an optional host source-loader callback. Returned
`Evaluation` entries expose their Lisp notation through `value()` and captured
text through `output`. The core performs no filesystem or network IO without a configured host;
only the CLI installs a filesystem loader.

Execution returns a value and captured output for each top-level expression,
without combining them into a transcript. The WASM response is UTF-8 JSON:
`{"evaluations":[{"value":"3","output":""}],"error":null}`. Values use Lisp
notation at this serialization boundary. On failure, `evaluations` is empty
and `error` contains the diagnostic, preserving the previous error-display
behavior. Earlier side effects, including definitions, are not rolled back.
The worker forwards the structured result; `web/results.js` formats the
browser transcript, retaining output/value ordering. Runtime state is one
of `loading`, `ready`, `running`, or `failed`, with both evaluation controls
derived from that state. `source_name_buffer`, `evaluate_named`, and
`register_source` extend the WASM ABI for named programs and user-selected
source files; `input_limit` and `source_limit` expose the runtime's bounds.
All three WASM request entrypoints share decoding, operation dispatch, response
serialization, and status handling. Their exported signatures and JSON response
shape remain unchanged.

The browser sends only new or edited source files before an evaluation.
The worker acknowledges each successful registration, including partial
success before an error; unacknowledged files are retried. Unchanged files are
not retransmitted. Reset, timeout, or worker replacement clears synchronization
state so all opened files are sent to the new runtime on its next evaluation.
The `completion_symbols` WASM export writes a JSON array of global names to
the output buffer without evaluating code. The worker refreshes suggestions
on initialization and after each request, including requests that fail after
creating bindings. Completion does not depend on the user-visible `symbols`
binding retaining its original value.

## Deployment

`.github/workflows/pages.yml` tests the interpreter, builds the WASM module and
static site, and deploys `dist/` through GitHub Pages Actions. Each build places
its CSS, JavaScript modules, worker, and WASM together under a content-addressed
`assets/` directory. The root HTML references that version, preventing cached
assets from an older release from breaking the new layout or runtime.
Pushes to `main`
trigger deployment. Pages must use **GitHub
Actions** as its source. All assets use relative URLs for the `/loop/` base path.
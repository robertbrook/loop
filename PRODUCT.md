# Loop

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

People writing Lisp programs and developing Loop as a language.

## Product Purpose

Develop Loop as its own Lisp dialect, not merely a demo or educational tool.
The browser workbench, examples, and REPL are interfaces to the language
implementation, not the limit of its intended use.

## Capabilities and Constraints

The interpreter is implemented in Rust and runs as WebAssembly. The public
deployment target is https://robertbrook.github.io/loop/. Loop takes inspiration
from Scheme but is its own dialect; Scheme or Common Lisp compatibility is not
its governing goal. The current implementation is an initial foundation:
future language features and compatibility commitments remain open.

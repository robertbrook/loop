#!/usr/bin/env bash
set -euo pipefail
cargo build --release --target wasm32-unknown-unknown
mkdir -p dist
cp web/* dist/
cp target/wasm32-unknown-unknown/release/loop_lisp.wasm dist/loop.wasm
touch dist/.nojekyll

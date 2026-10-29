#!/usr/bin/env bash
set -euo pipefail
cargo build --lib --release --target wasm32-unknown-unknown
export LC_ALL=C
wasm=target/wasm32-unknown-unknown/release/loop_lisp.wasm
revision=$(sha256sum web/* "$wasm" | sha256sum | cut -c1-16)
assets="dist/assets/$revision"
mkdir -p "$assets"
cp web/* "$assets/"
cp "$wasm" "$assets/loop.wasm"
sed -E "s@(src|href)=\"\\./@\\1=\"./assets/$revision/@g" web/index.html > dist/index.html
touch dist/.nojekyll

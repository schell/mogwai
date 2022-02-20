#! /bin/bash
set -e

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

echo "### CARGO TEST"
cargo test

echo "### CARGO DOC"
cargo doc

echo "### WASM-PACK TEST"
wasm-pack test --firefox --headless crates/mogwai --no-default-features --features dom-wasm
wasm-pack test --firefox --headless crates/mogwai-dom --no-default-features

echo "### WASM-PACK TEST HYDRATOR"
wasm-pack test --firefox --headless crates/mogwai-hydrator

echo "Done testing on ref ${GITHUB_REF}"

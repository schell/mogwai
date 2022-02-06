#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

echo "### CARGO TEST"
cargo test || exit 1

echo "### CARGO DOC"
cargo doc || exit 1

echo "### WASM-PACK TEST"
wasm-pack test --firefox --headless crates/mogwai || exit 1

echo "### WASM-PACK TEST HYDRATOR"
wasm-pack test --firefox --headless crates/mogwai-hydrator || exit 1

echo "Done testing on ref ${GITHUB_REF}"

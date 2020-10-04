#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cargo test || exit 1
cargo doc || exit 1
wasm-pack test --firefox --headless mogwai || exit 1

cargo build --package cookbook --target-dir cookbook_target --lib || exit 1
mdbook test cookbook -L cookbook_target/debug/deps || exit 1

echo "Done testing on ref ${GITHUB_REF}"

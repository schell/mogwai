#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cargo test || exit 1
cargo doc || exit 1

# only test headless at github
if [ -z ${GITHUB_REF+x} ]
then
    echo "Skipping headless wasm tests"
else
    wasm-pack test --firefox --headless mogwai || exit 1
    wasm-pack test --firefox --headless crates/mogwai-hydrator || exit 1
fi

echo "Done testing on ref ${GITHUB_REF}"

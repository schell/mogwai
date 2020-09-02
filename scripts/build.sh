#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cargo build || exit 1
cd mogwai
cargo publish --dry-run || exit 1
cd ..

echo "Done building on ref ${GITHUB_REF}"

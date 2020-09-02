#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cargo test || exit 1
cargo doc || exit 1

echo "Done testing on ref ${GITHUB_REF}"

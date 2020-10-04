#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

mdbook test cookbook -L target/debug/deps || (cargo clean && cargo build --package cookbook --lib && mdbook test cookbook -L target/debug/deps)

echo "Done testing on ref ${GITHUB_REF}"

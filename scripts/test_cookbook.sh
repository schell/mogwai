#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cargo clean && mdbook test cookbook -L target/debug/deps

echo "Done testing on ref ${GITHUB_REF}"

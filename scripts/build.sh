#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cargo build || exit 1

echo "Done building on ref ${GITHUB_REF}"

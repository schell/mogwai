#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cargo build || exit 1
for DIR in examples/*/
do
    echo ""
    echo "Building '${DIR}'"
    wasm-pack build --debug --target web $DIR
done

echo "Done building on ref ${GITHUB_REF}"

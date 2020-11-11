#!/bin/sh -eu

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

if [ -z ${GITHUB_REF+blah} ]; then
    GITHUB_REF="$(git rev-parse --abbrev-ref HEAD)"
fi

BRANCH=$(basename $GITHUB_REF)

echo "Testing project generation from the '$BRANCH' branch..."
cd ..
ls -lah mogwai-template
cargo generate --git ./mogwai-template --name gen-test
cd gen-test
wasm-pack build --target web

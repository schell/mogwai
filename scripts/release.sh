#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

TOKEN=$1

cd $ROOT
cd crates/mogwai-html-macro
cargo publish --dry-run --token $TOKEN

cd $ROOT
cd mogwai
cargo publish --dry-run --token $TOKEN

cd $ROOT
cd crates/mogwai-hydrator
cargo publish --dry-run --token $TOKEN

cd $ROOT

#! /bin/bash
set -e

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

TOKEN=$1

echo "publishing mogwai-html-macro"
cd $ROOT
cd crates/mogwai-html-macro
cargo publish --dry-run --token $TOKEN

echo "publishing mogwai"
cd $ROOT
cd mogwai
cargo publish --dry-run --token $TOKEN

echo "publishing mogwai-hydrator"
cd $ROOT
cd crates/mogwai-hydrator
cargo publish --dry-run --token $TOKEN

cd $ROOT

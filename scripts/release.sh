#! /bin/bash
set -e

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

TOKEN=$1
SLEEP="10s"

echo "publishing mogwai-html-macro"
cd $ROOT
cd crates/mogwai-html-macro
cargo publish --token $TOKEN
sleep $SLEEP
echo "  done!"

echo "publishing mogwai"
cd $ROOT
cd mogwai
cargo publish --token $TOKEN
sleep $SLEEP
echo "  done!"

echo "publishing mogwai-hydrator"
cd $ROOT
cd crates/mogwai-hydrator
cargo publish --token $TOKEN
echo "  done!"

cd $ROOT
mdbook build cookbook
aws s3 sync cookbook/book s3://zyghost.com/guides/mogwai-cookbook --acl public-read

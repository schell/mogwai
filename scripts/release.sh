#! /bin/bash
set -e

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

TOKEN=$1
SLEEP="10s"

echo "publishing mogwai-html-macro"
cd $ROOT
cd crates/mogwai-html-macro
#cargo publish --token $TOKEN
sleep 1s #$SLEEP
echo "  done!"

echo "publishing mogwai"
cd $ROOT
cd mogwai
#cargo publish --token $TOKEN
sleep 1s #$SLEEP
echo "  done!"

echo "publishing mogwai-hydrator"
cd $ROOT
cd crates/mogwai-hydrator
sleep $SLEEP
cargo publish --token $TOKEN

cd $ROOT

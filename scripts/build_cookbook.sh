#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cd $ROOT
. $ROOT/scripts/build.sh

export MDBOOK_preprocessor__variables__variables__cookbookroot="/guides/mogwai-cookbook"
mdbook build cookbook
mv $ROOT/book_examples cookbook/book/html/examples

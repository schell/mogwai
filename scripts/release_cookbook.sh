#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

cd $ROOT
mdbook build cookbook
aws s3 sync cookbook/book/html s3://zyghost.com/guides/mogwai-cookbook --acl public-read

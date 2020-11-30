#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/build_cookbook.sh

aws s3 sync cookbook/book/html s3://zyghost.com/guides/mogwai-cookbook --acl public-read

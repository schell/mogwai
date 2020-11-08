#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

TOKEN=$1
SLEEP="10s"

publish() {
    DIR=$1
    CRATE=`echo $DIR | cut -d'/' -f2-`
    echo "publishing $CRATE from $DIR"
    cargo search $CRATE
    cd $ROOT
    cd $DIR
    cargo publish --token $TOKEN
    if [ "$?" = "101" ]; then
        echo "  no dice!"
    else
        sleep $SLEEP
        echo "  done!"
    fi
    cd $ROOT
}

DIRS="crates/mogwai-html-macro mogwai crates/mogwai-hydrator"

for DIR in $DIRS; do
    publish $DIR
done

cd $ROOT
mdbook build cookbook
aws s3 sync cookbook/book s3://zyghost.com/guides/mogwai-cookbook --acl public-read

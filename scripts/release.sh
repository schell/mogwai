#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

TOKEN=$1
SLEEP="10s"

publish() {
    DIR=$1
    CRATE=`echo $DIR | cut -d'/' -f2-`
    echo "publishing $CRATE from $DIR"
    REMOTE_VERSION=`cargo search $CRATE | grep "$CRATE = " | cut -d'"' -f2`
    echo "  remote version: '$REMOTE_VERSION'"
    LOCAL_VERSION=`cat $DIR/Cargo.toml | grep -E '^version ?=' | cut -d$'\n' -f1 | cut -d'"' -f2`
    echo "  local version: '$LOCAL_VERSION'"
    if [ $REMOTE_VERSION = $LOCAL_VERSION ]; then
        echo "  cargo has the same version - done!"
    else
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
    fi
}

DIRS="crates/mogwai-html-macro mogwai crates/mogwai-hydrator"

for DIR in $DIRS; do
    publish $DIR
done

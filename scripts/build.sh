#! /bin/bash

ROOT="$(git rev-parse --show-toplevel)"
. $ROOT/scripts/common.sh

# create a dir to keep the examples for the book
mkdir -p $ROOT/book_examples

for DIR in examples/*/
do
    echo ""
    EXAMPLE=`echo $DIR | cut -d'/' -f2`
    echo "Building example project '${EXAMPLE}' from '${DIR}'"
    wasm-pack build --debug --target web $DIR
    if [ $DIR = "examples/multipage/" ]; then
        continue
    fi
    DEST=$ROOT/book_examples/$EXAMPLE
    mkdir -p $DEST
    cd $DIR
    cp -R index.html pkg style.css $DEST || exit 1
    cd $ROOT
done

echo "Done building on ref ${GITHUB_REF}"

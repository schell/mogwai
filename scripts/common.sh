#! /bin/bash
echo "### Common setup"

export PATH=$PATH:$HOME/.cargo/bin

if hash rustup 2>/dev/null; then
    echo " ## Have rustup, skipping installation..."
else
    echo " ## Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
fi

if hash wasm-pack 2>/dev/null; then
    echo " ## Have wasm-pack, skipping installation..."
else
    echo " ## Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

CARGO_BINS="mdbook mdbook-linkcheck mdbook-variables"
for BIN in $CARGO_BINS; do
    if hash $BIN 2>/dev/null; then
    echo " ## Have $BIN, skipping installation..."
    else
        echo " ## Installing $BIN..."
        cargo install $BIN
    fi
done

rustup toolchain install 1.46.0
rustup default 1.46.0

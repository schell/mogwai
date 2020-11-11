#!/bin/sh -eu

export PATH=$HOME/.cargo/bin:$PATH:

echo "Rust Setup"

if hash rustup 2>/dev/null; then
    echo "Have rustup, skipping installation..."
else
    echo "Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

rustup update

if hash wasm-pack 2>/dev/null; then
    echo "Have wasm-pack, skipping installation..."
else
    echo "Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

if hash cargo-generate 2>/dev/null; then
    echo "Have cargo-generate, skipping installation..."
else
    echo "Installing cargo-generate..."
    cargo install cargo-generate
    which cargo-generate
fi

#! /bin/bash
echo "### Common setup"

export PATH=$PATH:$HOME/.cargo/bin

if hash rustup 2>/dev/null; then
    echo " ## Have rustup, skipping installation..."
else
    echo " ## Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
fi

rustup toolchain install nightly
rustup default nightly

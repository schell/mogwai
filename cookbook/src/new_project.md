# Starting a new project
First you'll need a new(ish) version of the rust toolchain. For that you can visit
https://rustup.rs/ and follow the installation instructions.

## Browser apps

In most cases this cookbook assumes you'll be using [`mogwai-dom`][mogwai_dom] to build browser
applications.

For that you'll need [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) or
[trunk](https://trunkrs.dev/).

If you haven't set up a WASM project before it is recommended you read up on [Rust and
WebAssembly](https://rustwasm.github.io/docs/book/).

If you just want to quickly get hacking you may use the wonderful `cargo-generate`, which
can be installed using `cargo install cargo-generate`.

Then run
```shell
cargo generate --git https://github.com/schell/mogwai-template.git
```
and give the command line a project name. Then `cd` into your sparkling new
project and
```shell
wasm-pack build --target web
```
Then, if you don't already have it, `cargo install basic-http-server` or use your
favorite alternative to serve your app:
```shell
basic-http-server -a 127.0.0.1:8888
```

{{#include reflinks.md}}

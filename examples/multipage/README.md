# Multi Screen Example

An example that serves multiple "routes". Each "route" changes the main `View`
displayed by the `App` component.

The example performs SSR by injecting the `App` contents for the `Route` into
a [`tera`][tera] template. The SSR is not intended to be optimal, only
functional.

To compile the UI:

    wasm-pack build examples/multipage/ --target web

To start the server:

    cargo run --manifest-path=examples/multipage/Cargo.toml --features="server"

It is important to start the server from the project root, the `tera` templates
and the static files are located by relative path and the path is relative to
the execution directory rather than to the source directory.

[tera]: https://tera.netlify.app

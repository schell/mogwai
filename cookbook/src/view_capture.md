# 🕸️  Capturing parts of the View

Views often contain nodes that are required in the logic loop. When a view node is needed in a
logic loop we can capture it using a channel.

## Using the `capture:view` attribute

To capture a view after it is built you can use the [`capture:view`](rsx.md) attribute
with an `impl Sink<T>`, where `T` is your domain view type, and then await the first message on the
receiver:

```rust, ignore, no_run
{{#include ../../crates/mogwai-dom/src/lib.rs:capture_view_channel_md}}
```

## Using the [`Captured`][structcaptured] type

To make capturing a view easier you can use the [`Captured`][structcaptured] type, which encapsulates
the ends of a channel with a nice API:

```rust, ignore, no_run
{{#include ../../crates/mogwai-dom/src/lib.rs:capture_view_captured_md}}
```

{{#include reflinks.md}}

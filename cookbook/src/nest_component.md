# Nesting components
Any `impl Into<ViewBuilder<T>>` can be nested as a child node of another [ViewBuilder][structviewbuilder].

This includes:
- [ViewBuilder][structviewbuilder]
- [Component][structcomponent]
- [ElmComponent][structelmcomponent]
- `Option<impl Into<ViewBuilder<T>>>`
- `String`
- `&str`
- `(String, impl Stream<Item = String>)`
- `(&str, impl Stream<Item = String>)`

If there is a `std` type that you feel should have an `impl Into<ViewBuilder<T>>` please open
an issue at [the mogwai github repo](https://github.com/schell/mogwai/issues).

Here is a full example of nesting components:

```rust
{{#include ../../examples/nested-components/src/lib.rs:73:}}
```

{{#include reflinks.md}}

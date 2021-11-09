# Child components
Any `impl Into<ViewBuilder<T>>` can be nested as a child node of another [ViewBuilder][structviewbuilder].

This includes:
- [ViewBuilder][structviewbuilder]
- [Component][structcomponent]
- [ElmComponent][structelmcomponent]
- `Option<impl Into<ViewBuilder<T>>>`
- `Vec<impl Into<ViewBuilder<T>>>`
- `String`
- `&str`
- `(String, impl Stream<Item = String>)`
- `(&str, impl Stream<Item = String>)`

To nest a child component within a parent, simply include it as a node using RSX brackets:

```rust, no_run
# use mogwai::prelude::*;
let child = builder! {<p>"My paragraph"</p>};
let parent = builder! {<div>{child}</div>};
```

Or use the `ViewBuilder::append` function if you're not using RSX:
```rust, no_run
# use mogwai::prelude::*;
let child: ViewBuilder<Dom> = ViewBuilder::element("p")
    .append(ViewBuilder::text("My paragraph"));
let parent = ViewBuilder::element("div")
    .append(child);
```

If there is a `std` type that you feel should have an `impl Into<ViewBuilder<T>>` please open
an issue at [the mogwai github repo](https://github.com/schell/mogwai/issues).

{{#include reflinks.md}}

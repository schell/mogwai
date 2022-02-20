# Child components
Any `impl Into<ViewBuilder<T>>` can be nested as a child node of another [ViewBuilder][structviewbuilder].

This includes:
- [ViewBuilder][structviewbuilder]
- [Component][structcomponent]
- [ElmComponent][structelmcomponent]
- `String`
- `&str`
- `(String, impl Stream<Item = String>)`
- `(&str, impl Stream<Item = String>)`

Additionally some container/iterator types can be nested, with slightly different behavior:

- `Option<impl Into<ViewBuilder<T>>>` - if `None`, no child is added, otherwise if `Some(viewbuilder)`,
  `viewbuilder` is added. See [conditionally adding DOM](rsx.md#conditionally-include-dom).
- `Vec<impl Into<ViewBuilder<T>>>` - all children are appended one after another.
  See [including fragments](rsx.md#including-fragments)

To nest a child component within a parent, simply include it as a node using RSX brackets:

```rust, no_run
# use mogwai::prelude::*;
let child = html! {<p>"My paragraph"</p>};
let parent = html! {<div>{child}</div>};
```

Or use the `ViewBuilder::append` function if you're not using RSX:
```rust, no_run
# use mogwai::prelude::*;
let child: ViewBuilder<Dom> = ViewBuilder::element("p")
    .append(ViewBuilder::text("My paragraph"));
let parent = ViewBuilder::element("div")
    .append(child);
```

If there is a `std` type that you feel should have an `impl Into<ViewBuilder<T>>` or a container/iterator
that you'd like `mogwai` to support with regards to appending children, please open
an issue at [the mogwai github repo](https://github.com/schell/mogwai/issues).

{{#include reflinks.md}}

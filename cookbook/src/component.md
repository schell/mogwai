# Components
A "component", "widget", "gizmo" in `mogwai` is a view and zero or more async tasks that modify
that view. To create a view we use a [ViewBuilder][structviewbuilder]:

Components may have asynchronous tasks appended to them. When the view is built its tasks will
be spawned until they are complete, or when the view implementation decides to drop and cancel
them.

A view talks to its task loops using [sinks][traitsinkext] and [streams][traitstreamext].
This is the mode of `mogwai`.

Here is an example of a click counter:

```rust, ignore
{{#include ../../examples/nested-components/src/lib.rs:cookbook_components_counter}}
```

We can nest the counter component in another component:

```rust, ignore
{{#include ../../examples/nested-components/src/lib.rs:cookbook_components_app}}
```

And then build it all into one view:

```rust, ignore
{{#include ../../examples/nested-components/src/lib.rs:cookbook_components_app_build}}
```

{{#include reflinks.md}}

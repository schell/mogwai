# Components
The [Component][structcomponent] struct is used to compose logic and views together
into a user inteface. A blank `Component` can be created from a [ViewBuilder][structviewbuilder]:

```rust, ignore
{{#include ../../examples/nested-components/src/lib.rs:19:23}}
```

More complicated `Component`s have asynchronous logic appended to them. The view
talks to the logic loop using one or more channels. This is the mode of Mogwai.

Here we create a channel to send messages to the view from our logic loop:

```rust, ignore
{{#include ../../examples/nested-components/src/lib.rs:28:38}}
```

Then we can build the component, turning it into a [View][structview]:

```rust, ignore
{{#include ../../examples/nested-components/src/lib.rs:39:41}}
```

## 🕸️  Capturing parts of the View

Views often contain nodes that are required in the logic loop.

{{#include reflinks.md}}

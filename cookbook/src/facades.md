# 🏢 Facades and Relays 📞

Components talk within themselves from the view to the logic and vice versa, but there
are often stakeholders outside the component that would like to make queries, inject state,
or otherwise communicate with the internals of the component.

Because of this it is often convenient to provide a wrapper around view inputs and outputs
that have a nice API that breaks down requests from outside the component into its internal
messages. These wrappers can be called facades, or relays and they might be a bit familiar
if you've worked with MVC frameworks - but don't worry if you haven't! Mogwai is not a
framework or a philosophy and "whatever works is good".

## Inputs and Outputs instead of channels

To be a successful programmer of message passing systems, you have to know details about
the channels you're using to pass messages. You should know how much capacity they have
and what happens when you send messages above its capacity, if they clone, how they clone
as well as any other idiosyncracies of the channel. It can be quite a lot to wrangle!

To make this easier mogwai provides some channel wrappers in the [relay][modulerelay] module. These
wrappers provide a simplified API that makes working with a view's inputs and outputs easier.

* [Input][structinput] - an input to a view that has at most one consumer.
* [FanInput][structfaninput] - an input to a view that may have many consumers.
* [Output][structoutput] - an output from a view.

## Access to the raw view

You can access built, raw views by using [Captured][structcaptured]. Read more about this in
[Capturing parts of the view](view_capture.md).

## Model, ListPatchModel and HashPatchModel

You can share data between multiple components and react to streams of updates using the types in
the [model][modulemodel] module.

* [Model][structmodel] - wraps a shared `T` and provides a stream of the latest value to observers.
* [ListPatchModel][structlistpatchmodel] - wraps a vector of `T` and provides a stream of
  [ListPatch][enumlistpatch] to observers.
* [HashPatchModel][structhashpatchmodel] - wraps a hashmap of `K` keys and `V` values, providing a stream of
  [HashPatch][enumhashpatch] to observers.

## Helper struct

With these tools we can create a helper struct that can create our [`ViewBuilder`][structviewbuilder] and
then be used to control it through the API of our choosing, hiding the input/output implementation details.

A detailed example of this the
[`TodoItem`](https://github.com/schell/mogwai/blob/master/examples/todomvc/src/item.rs) from the todomvc
example project:

```rust, ignore
{{#include ../../examples/todomvc/src/item.rs}}
```

And here is the implementation, including the public API we will expose:

```rust, ignore
{{#include ../../examples/todomvc/src/item.rs:cookbook_facades_todoitem_impl}}
```

Notice above in `TodoItem::viewbuilder` that we have many more captured views, inputs and outputs than we
keep in `TodoItem`. This is because they are only needed for specific async tasks and are never accessed
from outside the `TodoItem`. These could have been included in `TodoItem` without harm and the choice of how to
structure your application is up to you.

{{#include reflinks.md}}

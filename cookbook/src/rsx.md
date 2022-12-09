# Rust Syntax Extension

Consider this variable declaration:

```rust
# use mogwai_dom::prelude::*;
let element = rsx!{ h1(){"Hello, world!"} };
```

This funny tag syntax is neither a string nor Rust code - it is a [`ViewBuilder`][structviewbuilder].

The macro `rsx!` is using RSX, which is a "**R**ust **S**yntax E**x**tension".
Similarly there is a `html!` macro that creates [`ViewBuilder`][structviewbuilder] from an HTML-ish
syntax:

```rust,
# use mogwai_dom::prelude::*;
let element = html!{ <h1>"Hello, world!"</h1> };
```

The two definitions are synonymous.

We recommend using these macros in mogwai to describe the DOM nodes used by your
components.
RSX cuts down on the amount of boilerplate you have to type when writing components.
RSX may remind you of a template language, but it comes with the full power of Rust.

You can always write your components without RSX - here is the same example above
written out manually:

```rust, no_run
# use mogwai_dom::prelude::*;
let my_builder: ViewBuilder = ViewBuilder::element("h1")
    .append(ViewBuilder::text("Hello, world!"));
```

## Tags
You may use any html tags you wish when writing RSX with `mogwai_dom`.

```rust, no_run
# use mogwai_dom::prelude::*;
let _: ViewBuilder = html! {
    <p>"Once upon a time in a galaxy far, far away..."</p>
};
```
## Attributes
Adding attributes happens the way you expect it to.
```rust, no_run
# use mogwai_dom::prelude::*;
let _: ViewBuilder = html! {
    <p id="starwars">"Once upon a time in a galaxy far, far away..."</p>
};
```
All html attributes are supported.

Attributes that have hyphens should be written with underscores.

### Special Mogwai Attributes
Additionally there are some `mogwai` specific attributes that do special things.
These are all denoted by two words separated by
a colon, with an expression on the right hand side. In most cases the right hand
side is allowed to be a `String`, an `&str`, an `impl Stream<Item = String>` or a
tuple of a stringy type and a string stream. See [MogwaiValue][enummogwaivalue]
for more details about types that can be turned into streams.

- **style:{name}** = `impl Into<MogwaiValue<String or &str, Stream<Item = String>>`

  Declares a single style.
  ```rust,no_run
  # use mogwai_dom::prelude::*;
  let _ = html! {
      <a href="#burritos" style:border="1px dashed #333">"link"</a>
  };
  ```

- **on:{event}** = `impl Sink<DomEvent>`

  Declares that the events of a certain type (`event`) occurring on the element should
  be sent on the given sender. You will often see the use of
  [Contravariant][traitcontravariant] in this position to map the type of the `Sender`.
  ```rust
  use mogwai_dom::prelude::*;
  use mogwai_dom::core::channel::broadcast;

  let (tx, _rx) = broadcast::bounded::<()>(1);
  let _ = html! {
      <div on:click=tx.contra_map(|_:DomEvent| ())>"Click me!"</div>
  };
  ```

- **window:{event}** = `impl Sink<DomEvent>`

  Declares that the windows's matching events should be sent on the given sender.
  ```rust
  use mogwai_dom::prelude::*;
  use mogwai_dom::core::channel::broadcast;

  # use mogwai_dom::prelude::*;
  let (tx, rx) = broadcast::bounded::<()>(1);
  let _ = html! {
      <div window:load=tx.contra_map(|_:DomEvent| ())>{("", rx.map(|()| "Loaded!".to_string()))}</div>
  };
  ```

- **document:{event}** = `impl Sink<DomEvent>`

  Declares that the document's matching events should be sent on the given transmitter.
  ```rust,no_run
  use mogwai_dom::prelude::*;
  use mogwai_dom::core::channel::broadcast;

  let (tx, rx) = broadcast::bounded::<String>(1);
  let _ = html! {
      <div document:keyup=tx.contra_map(|ev: DomEvent| format!("{:#?}", ev))>{("waiting for first event", rx)}</div>
  };
  ```

- **boolean:{name}** = `impl Into<MogwaiValue<bool, Stream<Item = bool>>`

  Declares a boolean attribute with the given name.
  ```rust,no_run
  # use mogwai_dom::prelude::*;
  let _ = html! {
      <input boolean:checked=true />
  };
  ```

- **patch:children** = `impl Stream<ListPatch<ViewBuilder>>`

  Declares that this element's children will be updated with a stream of [ListPatch][enumlistpatch].
  #### Note
  [ViewBuilder][structviewbuilder] is not `Clone`. For this reason we cannot use `mogwai::channel::broadcast::{Sender, Receiver}`
  channels to send patches, because a broadcast channel requires its messages to be `Clone`. Instead use
  `mogwai::channel::mpsc::{Sender, Receiver}` channels, which have no such requirement. Just remember that even though
  the `Receiver` can be cloned, if a `mpsc::Sender` has more than one `mpsc::Receiver`
  listening, only one will receive the message and the winning `Receiver` seems to alternate in round-robin style.
  ```rust, ignore
  {{#include ../../mogwai-dom/lib.rs:patch_children_rsx}}
  ```

- **post:build** = `FnOnce(&mut T)`

  Used to apply one-off changes to the domain specific view `T` after it has been built.

- **capture:view** = `impl Sink<T>`

  Used to capture a clone of the view after it has been built. The view type `T` must be `Clone`.
  For more info see [Capturing Views](view_capture.md)

- **cast:type** = Any domain specific inner view type, eg `Dom`

  Declares the inner type of the resulting [ViewBuilder][structviewbuilder]. By default this is
  [Dom][structdom].
  ```rust,ignore
  # use mogwai_dom::prelude::*;
  let my_input: ViewBuilder<MyCustomInnerView> = html! {
        <input cast:type=MyCustomInnerView />
  };
  ```

## Expressions
Rust expressions can be used as the values of attributes and as child nodes.
```rust, no_run
# use mogwai_dom::prelude::*;
let is_cool = true;
let _ = html! {
    <div>
        {
            if !is_cool {
                "This is hot."
            } else {
                "This is cool."
            }
        }
    </div>
};
```

## Conditionally include DOM

Within a tag or at the top level of an RSX macro, anything inside literal brackets is interpreted and used
as `Into<ViewBuilder<T>>`. Any type that can be converted into a [ViewBuilder][structviewbuilder]
can be used to construct a node including `Option<impl Into<ViewBuilder<T>>`. When the value is `None`,
an empty node is created.

Below we display a user's image if they have one:

```rust, ignore, no_run
{{#include ../../crates/mogwai-macros/tests/integration_test.rs:113:162}}
```

## Including fragments

You can use RSX to build more than one view at a time:

```rust, no_run
# use mogwai_dom::prelude::*;
// Create a vector with three builders in it.
let builders: Vec<ViewBuilder> = html! {
    <div>"hello"</div>
    <div>"hola"</div>
    <div>"kia ora"</div>
};

// Then add them all into a parent tag just like any component
let parent: ViewBuilder = html! {
    <section>{builders}</section>
};
```

## Without RSX

It is possible and easy to create mogwai views without RSX by using the
API provided by [ViewBuilder][structviewbuilder].

{{#include reflinks.md}}

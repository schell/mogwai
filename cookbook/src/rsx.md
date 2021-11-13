# Rust Syntax Extension

Consider this variable declaration:

```rust, no_run
# use mogwai::prelude::*;
let element = builder!{ <h1>"Hello, world!"</h1> };
```

This funny tag syntax is neither a string nor HTML - it is a [`ViewBuilder<Dom>`][structviewbuilder].

The macro `builder!` is using RSX, which is a "**R**ust **S**yntax E**x**tension".
Similarly there is a `view!` macro that creates [`View<Dom>`][structview].

```rust, no_run
# use mogwai::prelude::*;
let my_builder: ViewBuilder<Dom> = builder!{ <h1>"Hello, world!"</h1> };
let my_view: View<Dom> = view!{ <h1>"Hello, world!"</h1> };

let my_identical_view: View<Dom> = View::try_from(my_builder).unwrap();
```

We recommend using these macros in mogwai to describe the DOM nodes used by your
components.
RSX cuts down on the amount of boilerplate you have to type when writing components.
RSX may remind you of a template language, but it comes with the full power of Rust.

You can always write your components without RSX - here is the same example above
written out manually:

```rust, no_run
# use mogwai::prelude::*;
let my_builder: ViewBuilder<Dom> = ViewBuilder::element("h1")
    .with_child(ViewBuilder::text("Hello, world!"));
```

## Tags
You may use any html tags you wish when writing RSX.

```rust, no_run
# use mogwai::prelude::*;
let _: ViewBuilder<Dom> = builder! {
    <p>"Once upon a time in a galaxy far, far away..."</p>
};
```
## Attributes
Adding attributes happens the way you expect it to.
```rust, no_run
# use mogwai::prelude::*;
let _: ViewBuilder<Dom> = builder! {
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

- **style:{name}** = `impl Into<MogwaiValue<'a, String or &'a str, Stream<Item = String>>`

  Declares a single style.
  ```rust,no_run
  # use mogwai::prelude::*;
  let _ = builder! {
      <a href="#burritos" style:border="1px dashed #333">"link"</a>
  };
  ```

- **on:{event}** = `impl Sink<Event>`

  Declares that the events of a certain type (`event`) occurring on the element should
  be sent on the given sender. Since `web_sys::Event` is `!Send` and `!Sync` you will
  often see the use of [Contravariant][traitcontravariant] in this position, which
  allows passing around a channel that is `Send`.
  ```rust,no_run
  # use mogwai::prelude::*;
  let (tx, _rx) = broadcast::bounded::<()>(1);
  let _ = builder! {
      <div on:click=tx.sink().contra_map(|_:Event| ())>"Click me!"</div>
  };
  ```

- **window:{event}** = `impl Sink<Event>`

  Declares that the windows's matching events should be sent on the given sender.
  ```rust, no_run
  # use mogwai::prelude::*;
  let (tx, rx) = broadcast::bounded::<()>(1);
  let _ = builder! {
      <div window:load=tx.sink().contra_map(|_:Event| ())>{("", rx.map(|()| "Loaded!".to_string()))}</div>
  };
  ```

- **document:{event}** = `impl Sink<Event>`

  Declares that the document's matching events should be sent on the given transmitter.
  ```rust,no_run
  # use mogwai::prelude::*;
  let (tx, rx) = broadcast::bounded::<String>(1);
  let _ = builder! {
      <div document:keyup=tx.sink().contra_map(|ev| format!("{:#?}", ev))>{("waiting for first event", rx)}</div>
  };
  ```

- **boolean:{name}** = `impl Into<MogwaiValue<'a, bool, Stream<Item = bool>>`

  Declares a boolean attribute with the given name.
  ```rust,no_run
  # use mogwai::prelude::*;
  let _ = builder! {
      <input boolean:checked=true />
  };
  ```

- **patch:children** = `impl Stream<ListPatch<ViewBuilder<Dom>>>`

  Declares that this element's children will be updated with a stream of [ListPatch][enumlistpatch].
  #### Note
  [ViewBuilder][structviewbuilder] is not `Clone`. For this reason we cannot use `mogwai::channel::broadcast::{Sender, Receiver}`
  channels to send patches, because a broadcast channel requires its messages to be `Clone`. Instead use
  `mogwai::channel::mpmc::{Sender, Receiver}` channels, which have no such requirement. Just remember that even though
  the channels are technically "multi-producer and multi-consumer", if a `mpmc::Sender` has more than one `mpmc::Receiver`
  listening, only one will receive the message and the winning `Receiver` seems to alternate in round-robin style. So you
  are advised to use the `mpmc` channel as a "multi-producer, _single_ consumer" alternative to the broadcast channel.
  ```rust
  # use mogwai::prelude::*;
  let (tx, rx) = mpmc::bounded(1);
  let my_view = view! {
      <div id="main" patch:children=rx>"Waiting for a patch message..."</div>
  };

  smol::block_on(async {
    tx.send(ListPatch::drain()).await.unwrap();
    mpmc::until_empty(&tx).await; // just for testing - after this line the view has mutated the DOM
  });
  assert_eq!(String::from(&my_view), r#"<div id="main"></div>"#);

  let other_viewbuilder = builder! {
      <h1>"Hello!"</h1>
  };

  smol::block_on(async {
    tx.send(ListPatch::push(other_viewbuilder)).await.unwrap();
    mpmc::until_empty(&tx).await;
  });
  assert_eq!(String::from(&my_view), r#"<div id="main"><h1>Hello!</h1></div>"#);
  ```

- **cast:type** = Any domain specific inner view type, eg `Dom`

  Declares the inner type of the resulting [ViewBuilder][structviewbuilder]. By default this is
  [Dom][structdom].
  ```rust,ignore
  # use mogwai::prelude::*;
  let my_input: ViewBuilder<MyCustomInnerView> = builder! {
        <input cast:type=MyCustomInnerView />
  };
  ```

## Expressions
Rust expressions can be used as the values of attributes and as child nodes.
```rust, no_run
# use mogwai::prelude::*;
let is_cool = true;
let _ = builder! {
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
{{#include ../../crates/mogwai-html-macro/tests/integration_test.rs:113:162}}
```

## Including fragments

You can use RSX to build more than one view at a time:

```rust, no_run
# use mogwai::prelude::*;
// Create a vector with three builders in it.
let builders: Vec<ViewBuilder<Dom>> = builder! {
    <div>"hello"</div>
    <div>"hola"</div>
    <div>"kia ora"</div>
};

// Then add them all into a parent tag just like any component
let parent: ViewBuilder<Dom> = builder! {
    <section>{builders}</section>
};
```

## Without RSX

It is possible and easy to create mogwai views without RSX by using the
API provided by [ViewBuilder][structviewbuilder].

Here is the definition of `signed_in_user` above, written without RSX:

```rust, no_run
# use mogwai::prelude::*;
# struct User {
#    username: String,
#    o_image: Option<String>
# }
fn signed_in_view_builder(
    user: &User,
    home_class: impl Streamable<String>,
    editor_class: impl Streamable<String>,
    settings_class: impl Streamable<String>,
    profile_class: impl Streamable<String>,
) -> ViewBuilder<Dom> {
    let o_image: Option<ViewBuilder<Dom>> = user
        .o_image
        .as_ref()
        .map(|image| {
            if image.is_empty() {
                None
            } else {
                Some(
                    mogwai::builder::ViewBuilder::element("img")
                        .with_single_attrib_stream("class", "user-pic")
                        .with_single_attrib_stream("src", image)
                )
            }
        })
        .flatten();
    mogwai::builder::ViewBuilder::element("ul")
        .with_single_attrib_stream("class", "nav navbar-nav pull-xs-right")
        .append(
            mogwai::builder::ViewBuilder::element("li")
                .with_single_attrib_stream("class", "nav-item")
                .append(
                    mogwai::builder::ViewBuilder::element("a")
                        .with_single_attrib_stream("class", home_class)
                        .with_single_attrib_stream("href", "#/")
                        .append(mogwai::builder::ViewBuilder::text(" Home"))
                )
        )
        .append(
            mogwai::builder::ViewBuilder::element("li")
                .with_single_attrib_stream("class", "nav-item")
                .append(
                    mogwai::builder::ViewBuilder::element("a")
                        .with_single_attrib_stream("class", editor_class)
                        .with_single_attrib_stream("href", "#/editor")
                        .append(
                            mogwai::builder::ViewBuilder::element("i")
                                .with_single_attrib_stream("class", "ion-compose")
                        )
                        .append(mogwai::builder::ViewBuilder::text(" New Post"))
                )
        )
        .append(
            mogwai::builder::ViewBuilder::element("li")
                .with_single_attrib_stream("class", "nav-item")
                .append(
                    mogwai::builder::ViewBuilder::element("a")
                        .with_single_attrib_stream("class", settings_class)
                        .with_single_attrib_stream("href", "#/settings")
                        .append(
                            mogwai::builder::ViewBuilder::element("i")
                                .with_single_attrib_stream("class", "ion-gear-a")
                        )
                        .append(mogwai::builder::ViewBuilder::text(" Settings"))
                ),
        )
        .append(
            mogwai::builder::ViewBuilder::element("li")
                .with_single_attrib_stream("class", "nav-item")
                .append(
                    mogwai::builder::ViewBuilder::element("a")
                        .with_single_attrib_stream("class", profile_class)
                        .with_single_attrib_stream("href", format!("#/profile/{}", user.username))
                        .append(o_image)
                        .append(format!(" {}", user.username)),
                ),
        )
}
```

{{#include reflinks.md}}

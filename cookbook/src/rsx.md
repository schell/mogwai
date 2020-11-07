# Rust Syntax Extension

Consider this variable declaration:

```rust
# use mogwai::prelude::*;
let element = builder!{ <h1>"Hello, world!"</h1> };
```

This funny tag syntax is neither a string nor HTML - it is a `ViewBuilder<HtmlElement>`.

The macro `builder!` is using RSX, which is a "**R**ust **S**yntax E**x**tension".
Similarly there is a `view!` macro that creates `View<HtmlElement>`.

```rust
# use mogwai::prelude::*;
let my_builder: ViewBuilder<HtmlElement> = builder!{ <h1>"Hello, world!"</h1> };
let my_view: View<HtmlElement> = view!{ <h1>"Hello, world!"</h1> };

let my_identical_view: View<HtmlElement> = View::from(my_builder);
```

We recommend using these macros in mogwai to describe the DOM nodes used by your
components.
RSX cuts down on the amount of boilerplate you have to type when writing components.
RSX may remind you of a template language, but it comes with the full power of Rust.

## Tags
You may use any tags you wish when writing RSX.

```html
# use mogwai::prelude::*;
builder! {
    <p>"Once upon a time in a galaxy far, far away..."</p>
}
```
## Attributes
Adding attributes happens the way you expect it to.
```html
# use mogwai::prelude::*;
builder! {
    <p id="starwars">"Once upon a time in a galaxy far, far away..."</p>
}
```
All html attributes are supported.

### Special Mogwai Attributes
Additionally there are some `mogwai` specific
attributes that do special things. These are all denoted by two words separated by
a colon.

- **style:{name}** `= {expr: Into<Effect<String>>}`

  Declares a single style.
  ```rust,no_run
  # use mogwai::prelude::*;
  let _ = builder! {
      <a href="#burritos" style:border="1px dashed #333">"link"</a>
  };
  ```

- **on:{event}** `= {tx: &Transmitter<Event>}`

  Declares that the element's matching events should be sent on the given transmitter.
  ```rust,no_run
  # use mogwai::prelude::*;
  let (tx, _rx) = txrx::<()>();
  let _ = builder! {
      <div on:click=tx.contra_map(|_:&Event| ())>"Click me!"</div>
  };
  ```

- **window:{event}** = `= {tx: &Transmitter<Event>}`

  Declares that the windows's matching events should be sent on the given transmitter.
  ```rust,no_run
  # use mogwai::prelude::*;
  let (tx, rx) = txrx::<()>();
  let _ = builder! {
      <div window:load=tx.contra_map(|_:&Event| ())>{rx.branch_map(|_:&()| "Loaded!".to_string())}</div>
  };
  ```


- **document:{event}** = `= {tx: &Transmitter<Event>}`

  Declares that the document's matching events should be sent on the given transmitter.
  ```rust,no_run
  # use mogwai::prelude::*;
  let (tx, rx) = txrx::<Event>();
  let _ = builder! {
      <div document:keyup=tx>{rx.branch_map(|ev| format!("{:#?}", ev))}</div>
  };
  ```

- **boolean:{name}** `= {expr: Into<Effect<bool>}`

  Declares a boolean attribute with the given name.
  ```rust,no_run
  # use mogwai::prelude::*;
  let _ = builder! {
      <input boolean:checked=true />
  };
  ```

- **patch:children** `= {expr: Receiver<Patch<View<_>>>}`

  Declares that this element's children will be updated with [Patch][enumpatch] messages received on
  the given [Receiver][structreceiver].
  ```rust
  # use mogwai::prelude::*;
  let (tx, rx) = txrx();
  let my_view = view! {
      <div id="main" patch:children=rx>"Waiting for a patch message..."</div>
  };
  tx.send(&Patch::RemoveAll);

  let other_view = view! {
      <h1>"Hello!"</h1>
  };
  tx.send(&Patch::PushBack { value: other_view });

  assert_eq!(my_view.html_string(), r#"<div id="main"><h1>Hello!</h1></div>"#);
  ```

- **cast:type** `= web_sys::{type}`

  Declares that this element's underlying [DomNode][traitcomponent_atypedomnode] is the given type.
  ```rust,no_run
  # use mogwai::prelude::*;
  let my_input: ViewBuilder<web_sys::HtmlInputElement> = builder! {
        <input cast:type=web_sys::HtmlInputElement />
  };
  ```

## Transmitters, Receivers and Effects
[Transmitter][structtransmitter]s and [Receiver][structreceiver]s are the key to reactivity in
`mogwai`.

### Transmitters

[Transmitter][structtransmitter]s are given to a [View][structview] so that [View][structview]
can transmit DOM events out to any [Receiver][structreceiver]s. In most cases the
[Receiver][structreceiver] will be wired to the [Component::update][traitcomponent_methodupdate]
function. In turn, this function can send messages back out to that same [View][structview]'s
[Receiver][structreceiver]s.

### Receivers

[Receiver][structreceiver]s are responsible for the majority of DOM updates. It's possible to
patch the DOM by hand in the [Component::update][traitcomponent_methodupdate] function but in
most cases it's not necessary. [Receiver][structreceiver]s can be used as attribute values and
child nodes. When a message is received, that attribute will update its value, or that child node
will be replaced with the new message value.

### Effects
An [Effect][enumeffect] is either a value now or some values later, or both. We can declare
text nodes and attributes to have a value now and some values later using anything that can
be converted into an [Effect][enumeffect]. In most cases we'll use a tuple:
```rust,no_run
# use mogwai::prelude::*;
let (tx, rx_later) = txrx();
let _ = builder! {
    <div>
      <p>{("Value now!", rx_later.clone())}</p>
      <p>{"Only a value now!"}</p>
      <p>{rx_later}</p>
    </div>
};
tx.send(&"Value later".to_string());
```

## Expressions
Rust expressions can be used as the values of attributes and as child nodes.
```rust,no_run
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

## Casting the inner DOM element
You can cast the inner DOM element of a `View` or `ViewBuilder` using the special attribute `cast:type`:

```rust
use mogwai::prelude::*;
use web_sys::HtmlInputElement;

let name_input: View<HtmlInputElement> = view! {
    <input type="text" placeholder="Your Name" cast:type=web_sys::HtmlInputElement />
};
```

Without this explicit casting all DOM nodes assume the type `HtmlElement`;

## Conditionally include DOM

```rust
use mogwai::prelude::*;

struct User {
    username: String,
    o_image: Option<String>
}

fn signed_in_view_builder(
    user: &User,
    home_class: Effect<String>,
    editor_class: Effect<String>,
    settings_class: Effect<String>,
    profile_class: Effect<String>,
) -> ViewBuilder<HtmlElement> {
    let o_image: Option<ViewBuilder<HtmlElement>> = user
        .o_image
        .as_ref()
        .map(|image| {
            if image.is_empty() {
                None
            } else {
                Some(builder! { <img class="user-pic" src=image /> })
            }
        })
        .flatten();

    builder! {
        <ul class="nav navbar-nav pull-xs-right">
            <li class="nav-item">
                <a class=home_class href="#/">" Home"</a>
            </li>
            <li class="nav-item">
            <a class=editor_class href="#/editor">
                <i class="ion-compose"></i>
                " New Post"
                </a>
            </li>
            <li class="nav-item">
            <a class=settings_class href="#/settings">
                <i class="ion-gear-a"></i>
                " Settings"
                </a>
            </li>
            <li class="nav-item">
                <a class=profile_class href=format!("#/profile/{}", user.username)>
                    {o_image}
                    {format!(" {}", user.username)}
                </a>
            </li>
        </ul>
    }
}
```

## Without RSX

Here is the definition of `signed_in_user` above, written without RSX:

```rust
use mogwai::prelude::*;

struct User {
    username: String,
    o_image: Option<String>
}

fn signed_in_view_builder(
    user: &User,
    home_class: Effect<String>,
    editor_class: Effect<String>,
    settings_class: Effect<String>,
    profile_class: Effect<String>,
) -> ViewBuilder<HtmlElement> {
    let o_image: Option<ViewBuilder<HtmlElement>> = user
        .o_image
        .as_ref()
        .map(|image| {
            if image.is_empty() {
                None
            } else {
                Some({
                    let mut __mogwai_node = (ViewBuilder::element("img")
                        as ViewBuilder<web_sys::HtmlElement>);
                    __mogwai_node.attribute("class", "user-pic");
                    __mogwai_node.attribute("src", image);
                    __mogwai_node
                })
            }
        })
        .flatten();
    {
        let mut __mogwai_node = (ViewBuilder::element("ul")
            as ViewBuilder<web_sys::HtmlElement>);
        __mogwai_node.attribute("class", "nav navbar-nav pull-xs-right");
        __mogwai_node.with({
            let mut __mogwai_node = (ViewBuilder::element("li")
                as ViewBuilder<web_sys::HtmlElement>);
            __mogwai_node.attribute("class", "nav-item");
            __mogwai_node.with({
                let mut __mogwai_node = (ViewBuilder::element("a")
                    as ViewBuilder<web_sys::HtmlElement>);
                __mogwai_node.attribute("class", home_class);
                __mogwai_node.attribute("href", "#/");
                __mogwai_node.with(ViewBuilder::from(" Home"));
                __mogwai_node
            });
            __mogwai_node
        });
        __mogwai_node.with({
            let mut __mogwai_node = (ViewBuilder::element("li")
                as ViewBuilder<web_sys::HtmlElement>);
            __mogwai_node.attribute("class", "nav-item");
            __mogwai_node.with({
                let mut __mogwai_node = (ViewBuilder::element("a")
                    as ViewBuilder<web_sys::HtmlElement>);
                __mogwai_node.attribute("class", editor_class);
                __mogwai_node.attribute("href", "#/editor");
                __mogwai_node.with({
                    let mut __mogwai_node = (ViewBuilder::element("i")
                        as ViewBuilder<web_sys::HtmlElement>);
                    __mogwai_node.attribute("class", "ion-compose");
                    __mogwai_node
                });
                __mogwai_node.with(ViewBuilder::from(" New Post"));
                __mogwai_node
            });
            __mogwai_node
        });
        __mogwai_node.with({
            let mut __mogwai_node = (ViewBuilder::element("li")
                as ViewBuilder<web_sys::HtmlElement>);
            __mogwai_node.attribute("class", "nav-item");
            __mogwai_node.with({
                let mut __mogwai_node = (ViewBuilder::element("a")
                    as ViewBuilder<web_sys::HtmlElement>);
                __mogwai_node.attribute("class", settings_class);
                __mogwai_node.attribute("href", "#/settings");
                __mogwai_node.with({
                    let mut __mogwai_node = (ViewBuilder::element("i")
                        as ViewBuilder<web_sys::HtmlElement>);
                    __mogwai_node.attribute("class", "ion-gear-a");
                    __mogwai_node
                });
                __mogwai_node.with(ViewBuilder::from(" Settings"));
                __mogwai_node
            });
            __mogwai_node
        });
        __mogwai_node.with({
            let mut __mogwai_node = (ViewBuilder::element("li")
                as ViewBuilder<web_sys::HtmlElement>);
            __mogwai_node.attribute("class", "nav-item");
            __mogwai_node.with({
                let mut __mogwai_node = (ViewBuilder::element("a")
                    as ViewBuilder<web_sys::HtmlElement>);
                __mogwai_node.attribute("class", profile_class);
                __mogwai_node.attribute("href", format!("#/profile/{}", user.username));
                __mogwai_node.with(ViewBuilder::try_from({ o_image }).ok());
                __mogwai_node.with(
                    ViewBuilder::try_from(format!(" {}", user.username))
                    .ok(),
                );
                __mogwai_node
            });
            __mogwai_node
        });
        __mogwai_node
    }
}
```

{{#include reflinks.md}}

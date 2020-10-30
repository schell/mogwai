# Introducing RSX

Consider this variable declaration:

```rust
use mogwai::prelude::*;
let element = builder!{ <h1>"Hello, world!"</h1> };
```

This funny tag syntax is neither a string nor HTML - it is a `ViewBuilder<HtmlElement>`.

The macro `builder!` is using RSX, which is a "**R**ust **S**yntax E**x**tension".
Similarly there is a `view!` macro that creates `View<HtmlElement>`.

```rust
use mogwai::prelude::*;
let my_builder: ViewBuilder<HtmlElement> = builder!{ <h1>"Hello, world!"</h1> };
let my_view: View<HtmlElement> = view!{ <h1>"Hello, world!"</h1> };

let my_identical_view: View<HtmlElement> = View::from(my_builder);
```

We recommend using these macros in mogwai to describe what the UI should look like.
RSX cuts down on the amount of boilerplate you have to type when describing the UI.
RSX may remind you of a template language, but it comes with the full power of Rust.

## Tags

## Attributes

## Transmitters, Receivers and Effects

## Expressions

## Casting the inner DOM element

You can cast the inner DOM element using the special attribute `type:cast`:

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

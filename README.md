<div align="center">
  <h1>
      <img src="https://raw.githubusercontent.com/schell/mogwai/master/img/gizmo.svg" />
    <br />
    mogwai
  </h1>
</div>

> **m**inimal, **o**bvious, **g**raphical **w**idget **a**pplication **i**nterface

[![Crates.io][ci]][cl]

[ci]: https://img.shields.io/crates/v/mogwai.svg
[cl]: https://crates.io/crates/mogwai/

`mogwai` is a crate for creating GUI applications.

It is written in Rust and runs primarily in the browser, with features that also make it cross-platform.

- [Goals](#goals)
- [Concepts](#concepts)
- [Example](#example)
- [Intro](#introduction)
- [Why](#why)
- [Beginning](#ok---where-do-i-start)
- [Cookbook](#cookbook)
- [More examples](#more-examples)
- [Sponsor this project](#sponsorship)

## Goals

The goals of `mogwai` are to address the painful parts of building web apps with
[`web-sys`](https://crates.io/crates/web-sys).


Particularly:

* creating and styling elements and text
* event handling
* server-side rendering and cross-platform support

Additionally, `mogwai` doesn't try to provide an all-encompassing solution to building
GUI apps. You are free to mix and match your solutions.

## Concepts

The main concepts behind `mogwai` are minimalism and transparency.

The crate gives you a thin layer of tools and tries to get out of your way to
let you structure your app the way you want.

* **View construction**

  Constructing views is done using a novel `rsx!` macro that gives you the parts
  of the view that matter, while the rest are implicitly added.
  
* **Event occurrences are futures instead of callbacks** 

  Events like click, mouseover, etc. are handled by awaiting futures.

* **Cross-platform using traits**

  An interlocking system of traits are used to keep most operations cross-platform, while
  still allowing specific specialization when needed.

* **Idiomatic Rust** 

  Outside the `rsx!` macro, `mogwai` is idiomatic rust:
  - A widget is just a Rust type that contains the elements, text, event listeners and
    other state that you want grouped.
  - Events are just futures
  Figure out which patterns work for you.

## Example
Here is an example of a button that counts the number of times it has been clicked:

```rust, no_run
use mogwai::web::prelude::*;

#[derive(ViewChild)]
pub struct ButtonClick<V: View> {
    #[child]
    wrapper: V::Element,
    on_click: V::EventListener,
    clicks: Proxy<u32>,
}

impl<V: View> Default for ButtonClick<V> {
    fn default() -> Self {
        let mut proxy = Proxy::<u32>::default();

        rsx! {
            // create the outermost element and name it `wrapper`
            let wrapper = button(
                style:cursor = "pointer",
                // create an event listener for the "click" event, bound to
                // `on_click`
                on:click = on_click
            ) {
                // Create a text node with the current inner value of `proxy`
                // and add it to the view tree.
                // When `proxy` is updated, this node will automatically replaced
                // with another using the new value.
                {proxy(clicks => match *clicks {
                    1 => "Click again.".to_string(),
                    n => format!("Clicked {n} times."),
                })}
            }
        }

        Self {
            wrapper,
            clicks: proxy,
            on_click,
        }
    }
}

impl<V:View> ButtonClicks<V> {
    pub async fn step(&mut self) {
        let _ev = self.on_click.next().await;
        let current_clicks = self.clicks;
        self.clicks.set(current_clicks + 1);
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    let mut view = ButtonClick::<Web>::default();
    mogwai::web::body().append_child(&view);

    wasm_bindgen_futures::spawn_local(async move {
        loop {
            view.step().await;
        }
    });
}
```

## Introduction

If you're interested in learning more - please read the [introduction and
documentation](https://docs.rs/mogwai/latest/mogwai/web/an_introduction/index.html).

## Why

* No VDOM diffing keeps updates snappy and the implementation minimal
* A thin conceptual layer keeps you close to the metal, making it easy to get as low-level as
  you need.
* Async logic by default
* Explicit mutation
* `View` allows running on multiple platforms (web, server, ios, android, desktop, etc)

`mogwai` uses async Rust and a declarative `rsx!` macro for building views.

View mutation is explicit, and there is no performance overhead from vdom diffing.

### Made for rustaceans, by a rustacean

`mogwai` is a Rust first library. There is no requirement that you have `npm` or
`node`. Getting your project up and running without writing any javascript is easy
enough.

### Benchmarketing

`mogwai` is snappy! 

It's part of the on-going [`js-framework-benchemark`](https://krausest.github.io/js-framework-benchmark/).

`mogwai` is quite "fast" and it uses less memory than other WASM-based solutions.

## Ok - where do I start?

First you'll need the rust toolchain. For that you can visit
<https://rustup.rs/> and follow the installation instructions.

Then you'll need [trunk](https://trunkrs.dev/).

For starting a new mogwai project we'll use the wonderful `cargo-generate`, which
can be installed using `cargo install cargo-generate`.

Then run

```shell
cargo generate --git https://github.com/schell/mogwai-template.git
```

Follow the prompts to give the project a name.
Then `cd` into your new project and

```shell
trunk serve --config Trunk.toml
```

Happy hacking! :coffee: :coffee: :coffee:

## Cookbook

:green_book: [Cooking with Mogwai](https://zyghost.com/guides/mogwai-cookbook/index.html) is a series
of example solutions to various UI problems. It aims to be a good reference doc but not a step-by-step tutorial.

## Group channel :phone:

Hang out and talk about `mogwai` in the support channel:

* [Direct link to element app](https://app.element.io/#/room/#mogwai:matrix.org)
* invitation https://matrix.to/#/!iABugogSTxJNzlrcMW:matrix.org?via=matrix.org.

## More examples please

Examples can be found in [the examples' folder](https://github.com/schell/mogwai/blob/main/examples/).

To build the examples use:
```shell
trunk build --config examples/{the example}/Trunk.toml
```

Additional external examples include:
- [mogwai-realworld](https://github.com/schell/mogwai-realworld/) A "real world" app implementation (WIP)
- [the benchmark suite](https://github.com/schell/mogwai/blob/main/crates/mogwai-js-framework-benchmark)
- your example here ;)

## Sponsorship
Please consider sponsoring the development of this library!

* [Sponsor me on github](https://github.com/sponsors/schell/)

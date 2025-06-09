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

`mogwai` is a Rust crate for building GUI applications, primarily in the
browser, with cross-platform capabilities.

## Overview

`mogwai` simplifies web app development by addressing challenges with
[`web-sys`](https://crates.io/crates/web-sys), focusing on:

- Element creation and styling
- Event handling
- Server-side rendering and cross-platform support

It offers a minimalistic and transparent approach, allowing you to structure your app freely.

## Key Concepts

- **View Construction**: Use the `rsx!` macro for intuitive view building.
- **Event Handling**: Events are futures, not callbacks.
- **Cross-Platform**: Traits ensure operations are cross-platform, with room for specialization.
- **Idiomatic Rust**: Widgets are Rust types

## Example

Here's a button that counts clicks:

```rust, no_run
use mogwai::web::prelude::*;

#[derive(ViewChild)]
pub struct ButtonClick<V: View> {
    #[child]
    wrapper: V::Element,
    on_click: V::EventListener,
    num_clicks: Proxy<u32>,
}

impl<V: View> Default for ButtonClick<V> {
    fn default() -> Self {
        let mut num_clicks = Proxy::<u32>::default();

        rsx! {
            let wrapper = button(
                style:cursor = "pointer",
                on:click = on_click
            ) {
                // When the `num_clicks` proxy is updated it will replace this node.
                {num_clicks(n => match *n {
                    1 => "Click again.".to_string(),
                    n => format!("Clicked {n} times."),
                })}
            }
        }

        Self {
            wrapper,
            on_click,
            num_clicks,
        }
    }
}

impl<V: View> ButtonClick<V> {
    pub async fn step(&mut self) {
        let _ev = self.on_click.next().await;
        self.num_clicks.modify(|n| *n += 1);
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

## Getting Started

1. Install the Rust toolchain from <https://rustup.rs/>.
2. Install [trunk](https://trunkrs.dev/).
3. Use `cargo-generate` to start a new project:

```shell
cargo install cargo-generate
cargo generate --git https://github.com/schell/mogwai-template.git
cd your_project_name
trunk serve --config Trunk.toml
```

## Resources

- [Introduction and Documentation](https://docs.rs/mogwai/latest/mogwai/web/an_introduction/index.html)
- [Cooking with Mogwai](https://zyghost.com/guides/mogwai-cookbook/index.html)
- [Examples](https://github.com/schell/mogwai/blob/main/examples/)

## Community

Join the conversation on
[Element](https://app.element.io/#/room/#mogwai:matrix.org) or via
[Matrix](https://matrix.to/#/!iABugogSTxJNzlrcMW:matrix.org?via=matrix.org).

## Support

Consider [sponsoring on GitHub](https://github.com/sponsors/schell/) to support
development.

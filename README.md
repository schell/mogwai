# mogwai
`mogwai` is a minimalist, obvious, graphical web application interface written in
Rust that runs in your browser.

## goals

1. [x] easily declare static and dynamic markup, encapsulate state
2. [x] compile declarations into gizmos (mogwai's widgets)
3. [x] compose gizmos

If mogwai achieves these goals, maintaining application state, composing
widgets and reasoning about your program should be easy.

## example
```rust
extern crate mogwai;

use mogwai::prelude::*;

pub fn main() {
  let mut tx_click = Transmitter::new();
  let rx_text = Receiver::<String>::new();

  let button =
    button()
    .style("cursor", "pointer")
    .rx_text("Click me", rx_text.clone())
    .tx_on("click", tx_click.clone());

  tx_click.wire_fold(
    &rx_text,
    true, // our initial folding state
    |is_red, _| {
      let out =
        if *is_red {
          "Turn me blue".into()
        } else {
          "Turn me red".into()
        };
      (!is_red, Some(out))
    }
  );

  let rx_color = Receiver::<String>::new();

  let h1 =
    h1()
    .attribute("id", "header")
    .attribute("class", "my-header")
    .rx_style("color", "green", rx_color.clone())
    .text("Hello from mogwai!");

    tx_click.wire_fold(
      &rx_color,
      false, // the intial value for is_red
      |is_red, _| {
        let out =
          if *is_red {
            "blue".into()
          } else {
            "red".into()
          };
        (!is_red, Some(out))
    });

  div()
    .with(h1)
    .with(button)
    .build()
    .unwrap()
    .run()
    .unwrap();
}
```

## why
Rust is beginning to have a good number of frontend libraries. Most however,
encorporate a virtual DOM with a magical update phase. Even in a languague that
has performance to spare this step can cause unwanted slowness.

`mogwai` lives in a happy space between vdom and bare metal. It does this by
providing the tools needed to declare what in the DOM changes, and when. These
same tools encourage functional progamming patterns like encapsulation over
inheritance (or traits, in this case). It uses channel-like primitives and a
declarative html builder to define components and compose them together. Once the
interface is defined and built, the channels are effectively erased and it's
functions all the way down. There's no vdom, shadow dom, polling or patching -
just functions! So if you prefer a functional style of programming with lots of
maps and folds - or if you're looking to go `vroom!` then maybe `mogwai` is right
for you and your team :).

## made for rustaceans, by a rustacean
Another benefit of `mogwai` is that it is Rust-first. There is no requirement
that you have `npm` or `node`. Getting your project up and running without
writing any javascript is easy enough.

## performance
`mogwai` is snappy! Here is a very handwavey and sketchy todomvc benchmark:

![mogwai performance benchmarking](img/perf.png)

## ok - where do i start?
`mogwai` is meant to be used with [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/).

(more detailed instructions incoming)

## more examples please
For more examples, check out
[the sandbox](https://github.com/schell/mogwai/blob/master/mogwai-sandbox/src/lib.rs).
[the todomvc app](https://github.com/schell/mogwai-todo)

To build the sandbox use:
```bash
cd mogwai-sandbox; wasm-pack build --target no-modules
```

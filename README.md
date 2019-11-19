# mogwai
`mogwai` is the magical, obvious, graphical web application interface! `mogwai`
is written in Rust but it runs in your browser.

# goals

1. [x] be able to easily declare static markup
2. [x] be able to easily declare dynamic markup
3. [x] declaring static markup, dynamic markup and controlling updates to
       dynamic markup in a localized, stateful way is the act of writing a
       widget, which in `mogwai` is called a gizmo
4. [x] gizmos are composable

If mogwai achieves these goals, maintaining application state, composing
widgets and reasoning about your program should be easy.

# example
```rust
extern crate mogwai;

use mogwai::prelude::*;

pub fn main() {
  let mut tx_click = Transmitter::new();
  let rx_text = Receiver::<String>::new();

  let button =
    button()
    .text("Click me")
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


  // Here we're not unwrapping because this readme is tested without compiling
  // to wasm. In an actual wasm application your main would look a little
  // different. See mogwai-sandbox/src/lib.rs
  div()
    .with(h1)
    .with(button)
    .build()
    .unwrap()
    .run()
    .unwrap();
}
```

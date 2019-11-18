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
  let app:GizmoBuilder = {
    let mut tx_click = Transmitter::<()>::new();
    let mut rx_text = Receiver::<String>::new();

    let mut button:GizmoBuilder =
      button()
      .text("Click me")
      .style("cursor", "pointer")
      .rx_text("Click me", rx_text.clone());

    button.tx_on("click", tx_click.clone());

    wire(
      &mut tx_click,
      &mut rx_text,
      true, // our initial folding state
      |is_red, &()| {
        let out =
          if *is_red {
            "Turn me blue".into()
          } else {
            "Turn me red".into()
          };
        (!is_red, Some(out))
      }
    );

    let mut rx_color = Receiver::<String>::new();

    let mut h1:GizmoBuilder =
      h1()
      .id("header")
      .class("my-header")
      .rx_style("color", "green", rx_color.clone())
      .text("Hello from mogwai!");

     wire(
      &mut tx_click,
      &mut rx_color,
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
      .build()?
  };

  app.run();
```

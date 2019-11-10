# mogwai
`mogwai` is the magical, obedient, graphical web application interface! `mogwai`
is written in Rust but it runs in your browser. It takes inspiration from
declarative blah blah.

# goals

1. [x] be able to easily declare static markup
2. [ ] be able to easily declare dynamic markup, and in turn provide mutable
       references to dynamic markup for later updates
3. [ ] declaring static markup, dynamic markup and controlling updates to
       dynamic markup in a localized, stateful way is the act of writing a
       widget, which in `mogwai` is called a gizmo
4. [ ] gizmos are composable
5. [ ] when widgets fall out of scope, their respective static and dynamic
       markup does too

If mogwai achieves these goals, maintaining application state, composing
widgets and reasoning about your program should be easy.

# example
```rust
  let app:Gizmo = {
    let mut h1:Gizmo =
      GizmoBuilder::h1()
      .id("header")
      .class("my-header")
      .text("Hello from mogwai!")
      .build();

    let mut button:Gizmo =
      GizmoBuilder::button()
      .text("Click me")
      .build();

    let click:Event<()> =
      button.on_click();

    let dyn_color:Dynamic<String> =
      click
      .fold_into(
        "red".to_string(),
        |last_color, ()| {
          if &last_color == "red" {
            "blue"
          } else {
            "red"
          }
        }
      );

    let dyn_btn_text:Dynamic<String> =
      dyn_color
      .clone()
      .map(|color:String| -> String {
        let nxt:String =
          if &color == "red" {
            "blue"
          } else {
            "red"
          }.to_string();
        format!("Turn back to {:?}", color)
      });

    h1.style("color", dyn_color);
    button.text(dyn_btn_text);

    GizmoBuilder::main()
      .with(h1)
      .with(button)
      .build()
  };

  app.maintain();
```

#[macro_use]
extern crate log;
extern crate console_log;
extern crate web_sys;
extern crate mogwai_design;

use wasm_bindgen::prelude::*;
use log::Level;
use mogwai_design::*;
use web_sys::{Element, Document, HtmlElement, Node, Text, window};
//use std::thread::sleep;
//use std::time::Duration;


#[wasm_bindgen]
pub fn main() -> Result<(), JsValue> {
  console_log::init_with_level(Level::Trace)
    .unwrap();
  trace!("Hello from mogwai");

  let window =
    window()
    .expect("Could not access the window");

  let document =
    window
    .document()
    .expect("Could not access the document");

  // Goals:
  // 1. [x] be able to easily declare static markup
  // 2. [ ] be able to easily declare dynamic markup, and in turn provide mutable
  //        references to dynamic markup for later updates
  //let mut dyn_color:Dynamic<String> =
  //  Dynamic::new("red".into());
  let app:Gizmo = {
    let mut h1:Gizmo =
      h1()
      .id("header")
      .class("my-header")
      .text("Hello from mogwai!")
      .build();

    let mut button:Gizmo =
      button()
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

  // 3. [ ] declaring static markup, dynamic markup and controlling updates to
  //        dynamic markup in a localized, stateful way is the act of writing a
  //        widget
  // 4. [ ] widgets are composable
  // 5. [ ] when widgets fall out of scope, their respective static and dynamic
  //        markup does too

  document
    .body()
    .unwrap()
    .append_child(&app)
    .unwrap();

  app.run();

  Ok(())
}

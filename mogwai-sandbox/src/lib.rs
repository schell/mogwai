#[macro_use]
extern crate log;
extern crate console_log;
extern crate web_sys;
extern crate mogwai_design;

use log::Level;
use mogwai_design::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Element, Document, HtmlElement, Node, Text, window};
//use std::thread::sleep;
//use std::time::Duration;


#[wasm_bindgen]
pub fn main() -> Result<(), JsValue> {
  console_log::init_with_level(Level::Trace)
    .unwrap();
  trace!("Hello from mogwai");

  let app:Gizmo = {
    let mut dyn_color: Dynamic<String> =
      Dynamic::new("green");

    let h1:Gizmo =
      h1()
      .attribute("id", "header")
      .attribute("class", "my-header")
      .style_dyn("color", dyn_color.clone())
      .text("Hello from mogwai!")
      .build()?;

    let mut dyn_btn_text:Dynamic<String> =
      Dynamic::new("Click me");
    let mut button:Gizmo =
      button()
      .text_dyn(dyn_btn_text.clone())
      .build()?;

    let click:Event<()> =
      button.on("click");
    let dyn_clicks:Dynamic<u32> =
      click
      .fold_into(0, |n, ()| n + 1);

    dyn_color
      .replace_with(
        dyn_clicks
          .clone()
          .map(|n| {
            if n % 2 == 0 {
              "red"
            } else {
              "blue"
            }.to_string()
          })
      );

    dyn_btn_text
      .replace_with(
        dyn_clicks
          .clone()
          .map(|n| {
            let color =
              if n % 2 == 0 {
                "blue"
              } else {
                "red"
              }.to_string();
            format!("Turn back to {}", color)
          })
      );

    div()
      .with(h1)
      .with(button)
      .build()?
  };

  trace!("Done building...");

  app.run()
}

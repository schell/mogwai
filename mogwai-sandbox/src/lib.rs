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
use web_sys::{Element, Event, Document, HtmlElement, Node, Text, window};
//use std::thread::sleep;
//use std::time::Duration;


#[wasm_bindgen]
pub fn main() -> Result<(), JsValue> {
  console_log::init_with_level(Level::Trace)
    .unwrap();
  trace!("Hello from mogwai");

  let app:Gizmo = {
    //// Create some hookups to change the text of our h1
    //let (tx_btn_click, mut btn_click_to_h1_color, rx_h1_color) =
    //  Wire::<Event, String>::hookups();

    //let h1:Gizmo =
    //  h1()
    //  .attribute("id", "header")
    //  .attribute("class", "my-header")
    //  // Pass the receiving end into the gizmo along with an initial value
    //  .style_rx("color", "green", rx_h1_color)
    //  .text("Hello from mogwai!")
    //  .build()?;

    //// Extend our existing wire so that input on `tx_btn_click` will also update
    //// a new wire and a new receiver.
    //let (btn_click_to_btn_text, rx_btn_text) =
    //  btn_click_to_h1_color
    //  .extend();
    //let mut button:Gizmo =
    //  button()
    //  .text_rx("Click me", rx_btn_text)
    //  .build()?;

    //let click:Event<()> =
    //  button.on("click");
    //let dyn_clicks:Dynamic<u32> =
    //  click
    //  .fold_into(0, |n, ()| n + 1);

    //dyn_color
    //  .replace_with(
    //    dyn_clicks
    //      .clone()
    //      .map(|n| {
    //        if n % 2 == 0 {
    //          "red"
    //        } else {
    //          "blue"
    //        }.to_string()
    //      })
    //  );

    //dyn_btn_text
    //  .replace_with(
    //    dyn_clicks
    //      .clone()
    //      .map(|n| {
    //        let color =
    //          if n % 2 == 0 {
    //            "blue"
    //          } else {
    //            "red"
    //          }.to_string();
    //        format!("Turn back to {}", color)
    //      })
    //  );

    //div()
    //  .with(h1)
    //  .with(button)
    //  .build()?
    panic!("")
  };

  trace!("Done building...");

  app.run()
}

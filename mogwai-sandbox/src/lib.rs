#[macro_use]
extern crate log;
extern crate console_log;
extern crate console_error_panic_hook;
extern crate web_sys;
extern crate mogwai_design;

use log::Level;
use wasm_bindgen::prelude::*;
use mogwai_design::prelude::*;
use std::panic;


/// Defines a button that changes its text every time it is clicked.
/// Once built, the button will also transmit clicks into the given transmitter.
pub fn new_button_gizmo(mut tx_click: InstantTransmitter<()>) -> GizmoBuilder {
  // Create a receiver for our button to get its text from.
  let mut rx_text = InstantReceiver::<String>::new();

  // Create the button that gets its text from our receiver.
  //
  // The button text will start out as "Click me" and then change to whatever
  // comes in on the receiver.
  let mut button =
    button()
    .named("button")
    .rx_text("Click me", rx_text.clone());

  // Have the button transmit on tx_click
  button.tx_on("click", tx_click.clone());

  // Now that the routing is done, we can define how the signal changes from
  // transmitter to receiver over each occurance.
  // We do this by wiring the two together, along with some internal state in the
  // form of a fold function.
  wire(
    &mut tx_click,
    &mut rx_text,
    true, // our initial folding state
    |is_red, _| {
      trace!("button::tx_click->rx_text");
      trace!("  last is_red:{}", is_red);
      let out =
        if *is_red {
          "Turn me blue".into()
        } else {
          "Turn me red".into()
        };
      trace!("  out:{:?}", out);
      (!is_red, Some(out))
    }
  );

  button
}


/// Creates a h1 heading that changes its color.
pub fn new_h1_gizmo(mut tx_click:InstantTransmitter<()>) -> GizmoBuilder {
  // Create a receiver for our heading to get its color from.
  let mut rx_color = InstantReceiver::<String>::new();

  // Create the builder for our heading, giving it the receiver.
  let h1:GizmoBuilder =
    h1()
    .named("h1")
    .attribute("id", "header")
    .attribute("class", "my-header")
    .rx_style("color", "green", rx_color.clone())
    .text("Hello from mogwai!");

  // Now that the routing is done, let's define the logic
  // The h1's color will change every click back and forth between blue and red
  // after the initial green.
  wire(
    &mut tx_click,
    &mut rx_color,
    false, // the intial value for is_red
    |is_red, _| {
      trace!("h1::tx_click->rx_color");
      let out =
        if *is_red {
          "blue".into()
        } else {
          "red".into()
        };
      (!is_red, Some(out))
    });

  h1
}


#[wasm_bindgen]
pub fn main() -> Result<(), JsValue> {
  panic::set_hook(Box::new(console_error_panic_hook::hook));
  console_log::init_with_level(Level::Trace)
    .unwrap();
  trace!("Hello from mogwai");

  // Create a transmitter to send button clicks into.
  let tx_click = InstantTransmitter::<()>::new();
  let h1 = new_h1_gizmo(tx_click.clone());
  let btn = new_button_gizmo(tx_click);

  // Put it all in a parent gizmo and run it right now
  div()
    .named("root_div")
    .with(h1)
    .with(btn)
    .build()?
    .run()
}

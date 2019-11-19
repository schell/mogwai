#[macro_use]
extern crate log;
extern crate console_log;
extern crate console_error_panic_hook;
extern crate mogwai;

use log::Level;
use wasm_bindgen::prelude::*;
use mogwai::prelude::*;
use std::panic;
use std::sync::{Arc, Mutex};


/// Defines a button that changes its text every time it is clicked.
/// Once built, the button will also transmit clicks into the given transmitter.
pub fn new_button_gizmo(mut tx_click: Transmitter<Event>) -> GizmoBuilder {
  // Create a receiver for our button to get its text from.
  let mut rx_text = Receiver::<String>::new();

  // Create the button that gets its text from our receiver.
  //
  // The button text will start out as "Click me" and then change to whatever
  // comes in on the receiver.
  let button =
    button()
    .named("button")
    .style("cursor", "pointer")
    // The button receives its text
    .rx_text("Click me", rx_text.clone())
    // The button transmits its clicks
    .tx_on("click", tx_click.clone());

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
pub fn new_h1_gizmo(mut tx_click:Transmitter<Event>) -> GizmoBuilder {
  // Create a receiver for our heading to get its color from.
  let mut rx_color = Receiver::<String>::new();

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


/// Creates a button that when clicked, requests duckduckgo.com and sends
/// it down a receiver.
pub fn time_req_button_and_pre() -> GizmoBuilder {
  let (req_tx, mut req_rx) = terminals::<Event>();
  let (resp_tx, resp_rx) = terminals::<String>();

  let may_promise =
    Arc::new(Mutex::new(None));
  req_rx
    .set_responder(move |_| {
      let is_in_flight =
        may_promise
        .try_lock()
        .unwrap()
        .is_some();
      if !is_in_flight {

        let send_resp_tx = resp_tx.clone();
        let inner_may_promise =
          may_promise.clone();
        let send_closure =
          Arc::new(
            Closure::wrap(Box::new(move |text_value:JsValue| {
              text_value
                .as_string()
                .into_iter()
                .for_each(|text| {
                  send_resp_tx.send(&text);

                  *inner_may_promise
                    .try_lock()
                    .unwrap()
                    = None;
                })
            }) as Box<FnMut(JsValue)>)
          );

        let inner_send_closure =
          send_closure.clone();
        let resp_closure =
          Arc::new(
            Closure::wrap(Box::new(move |resp_value:JsValue| {
              resp_value
                .dyn_into::<Response>()
                .unwrap()
                .text()
                .unwrap()
                .then(&inner_send_closure);
            }) as Box<FnMut(JsValue)>)
          );

        let mut opts =
          RequestInit::new();
        opts.method("GET");
        opts.mode(RequestMode::Cors);

        let req =
          Request::new_with_str_and_init(
            "https://worldtimeapi.org/api/timezone/Europe/London.txt",
            &opts
          )
          .unwrap();

        let promise =
          window()
          .fetch_with_request(&req)
          .then(&resp_closure);

        *may_promise
          .try_lock()
          .unwrap()
          = Some((promise, send_closure, resp_closure));
      } else {
        warn!("Attempted to start a new request");
      }
    });

  let btn =
    button()
    .named("request_button")
    .style("cursor", "pointer")
    .text("Find the time in london")
    .tx_on("click", req_tx);
  let pre =
    GizmoBuilder::new("pre")
    .named("request_pre")
    .rx_text("Awaiting request...", resp_rx);
  div()
    .with(btn)
    .with(pre)
}


#[wasm_bindgen]
pub fn main() -> Result<(), JsValue> {
  panic::set_hook(Box::new(console_error_panic_hook::hook));
  console_log::init_with_level(Level::Trace)
    .unwrap();
  trace!("Hello from mogwai");

  // Create a transmitter to send button clicks into.
  let tx_click = Transmitter::new();
  let h1 = new_h1_gizmo(tx_click.clone());
  let btn = new_button_gizmo(tx_click);
  let req = time_req_button_and_pre();

  // Put it all in a parent gizmo and run it right now
  div()
    .named("root_div")
    .with(h1)
    .with(btn)
    .with(GizmoBuilder::new("br"))
    .with(GizmoBuilder::new("br"))
    .with(req)
    .build()?
    .run()
}

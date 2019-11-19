use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::Arc;
use wasm_bindgen::closure::Closure;
use web_sys::{HtmlElement, Node, Text};

use super::prelude::*;
pub use super::utils::*;
pub use web_sys::EventTarget;
pub use wasm_bindgen::{JsCast, JsValue};

/// Things we can take advantage of:
/// * javascript is single threaded (wasm may not be in the future)
/// * wasm updates are typically like set_property(&self, name, value) -> Result<(), JsValue>
///   (they don't mutate)
/// * clones of elements actually reference the same DOM element

#[derive(Clone)]
pub struct Gizmo {
  pub name: String,
  html_element: HtmlElement,
  callbacks: HashMap<String, Arc<Closure<FnMut(JsValue)>>>,
  pub sub_gizmos: Vec<Gizmo>,
}


impl Gizmo {
  pub fn new(html_element: HtmlElement) -> Gizmo {
    Gizmo {
      name: "unknown".into(),
      html_element,
      callbacks: HashMap::new(),
      sub_gizmos: vec![],
    }
  }

  /// Sends an event into the given transmitter when the given dom event happens.
  pub fn tx_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
    let target:&EventTarget =
      self
      .html_element
      .dyn_ref()
      .expect("Could not get element EventTarget");

    let name = self.name.clone();
    let cb =
      Closure::wrap(Box::new(move |val:JsValue| {
        trace!("{} - an event happened!", name);
        // TODO: Do something with the js event
        // push the value into the sender
        let ev =
          val
          .dyn_into()
          .expect("Callback was not an event!");
        tx.send(&ev);
      }) as Box<FnMut((JsValue))>);
    target
      .add_event_listener_with_callback(ev_name, cb.as_ref().unchecked_ref())
      .unwrap();
    self
      .callbacks
      .insert(ev_name.to_string(), Arc::new(cb));
  }

  /// Sends a message into the given transmitter repeatedly at the given interval.
  /// Stops sending as soon as a message is received on the given receiver.
  /// The interval is defined in milliseconds.
  pub fn tx_interval(&mut self, _millis: u32, _tx: Transmitter<()>, _rx: Receiver<()>) {
    //let mut rx = rx.branch();
    //let f = Closure::wrap(Box::new(|| {

    //}));
  }

  pub fn attribute(&mut self, name: &str, init: &str, mut rx: Receiver<String>) {
    self
      .html_element
      .set_attribute(name, init)
      .expect("Could not set attribute");
    let el = self.html_element.clone();
    let name = name.to_string();
    rx.set_responder(move |s| {
      el.set_attribute(&name, s)
        .expect("Could not set attribute");
    });
  }

  pub fn text(&mut self, init: &str, mut rx: Receiver<String>) {
    let text:Text =
      Text::new_with_data(init)
      .unwrap();
    self
      .html_element
      .dyn_ref::<Node>()
      .expect("Could not convert gizmo element into a node")
      .append_child(text.as_ref())
      .expect("Could not add text node to gizmo element");
    rx.set_responder(move |s| {
      text.set_data(s);
    });
  }

  pub fn style(&mut self, s: &str, init: &str, mut rx: Receiver<String>) {
    let style =
      self
      .html_element
      .dyn_ref::<HtmlElement>()
      .expect("Could not cast Element into HtmlElement")
      .style();

    let name =
      s.to_string();

    style
      .set_property(&name, init)
      .expect("Could not set initial style property");

    rx.set_responder(move |s| {
      style
        .set_property(&name, s)
        .expect("Could not set style");
    });
  }

  pub fn with(&mut self, init: Gizmo, mut rx: Receiver<GizmoBuilder>) {
    let mut prev_gizmo = init;
    let node =
      self
      .html_element
      .clone()
      .dyn_into::<Node>()
      .expect("Could not turn gizmo html_element into Node");
    node
      .append_child(prev_gizmo.html_element_ref())
      .expect("Could not add initial child gizmo");
    rx.set_responder(move |gizmo_builder: &GizmoBuilder| {
      let gizmo =
        gizmo_builder
        .build()
        .expect("Could not build dynamic gizmo");

      let prev_node:&Node =
        prev_gizmo
        .html_element
        .dyn_ref()
        .expect("Could not cast old dynamic gizmo's html_element into node");

      let new_node:&Node =
        &gizmo
        .html_element
        .dyn_ref()
        .expect("Could not cast dynamic gizmo's html_element into node");

      node
        .replace_child(new_node, prev_node)
        .expect("Could not replace old gizmo with new gizmo");

      prev_gizmo = gizmo;
    });
  }

  pub fn html_element_ref(&self) -> &HtmlElement {
    &self.html_element
  }

  pub fn maintain(&mut self) {}

  pub fn run(self) -> Result<(), JsValue> {
    trace!("Running gizmo {}...", self.name);

    body()
      .append_child(self.html_element_ref())
      .unwrap();

    let gizmo = RefCell::new(self);

    timeout(1000, move || {
      // TODO: Use the "main loop" interval to sync stats
      // ...about the gizmo graph and wirings of gizmos.
      gizmo.borrow_mut().maintain();
      true
    });

    Ok(())
  }
}

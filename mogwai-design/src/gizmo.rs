use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen::closure::Closure;
use web_sys::{Event, EventTarget, HtmlElement, Node, Text};

use super::prelude::*;

/// Things we can take advantage of:
/// * javascript is single threaded (wasm may not be in the future)
/// * wasm updates are typically like set_property(&self, name, value) -> Result<(), JsValue>
///   (they don't mutate)
/// * clones of elements actually reference the same DOM element

#[derive(Clone)]
pub struct Gizmo {
  pub name: String,
  html_element: HtmlElement,
  callbacks: HashMap<String, Arc<Closure<FnMut(Event)>>>,
  pub sub_gizmos: Vec<Gizmo>,
  pub fuse_box: FuseBox,
}


impl Gizmo {
  pub fn new(html_element: HtmlElement) -> Gizmo {
    Gizmo {
      name: "unknown".into(),
      html_element,
      callbacks: HashMap::new(),
      fuse_box: FuseBox::new(),
      sub_gizmos: vec![],
    }
  }

  /// Sends an event into the given transmitter when the given dom event happens.
  pub fn tx_on(&mut self, ev_name: &str, mut tx: InstantTransmitter<()>) {
    let target:&EventTarget =
      self
      .html_element
      .dyn_ref()
      .expect("Could not get element EventTarget");

    let name = self.name.clone();
    let cb =
      Closure::wrap(Box::new(move |_ev:Event| {
        trace!("{} - an event happened!", name);
        // TODO: Do something with the js event
        // push the value into the sender
        tx.send(&());
      }) as Box<FnMut((Event))>);
    target
      .add_event_listener_with_callback(ev_name, cb.as_ref().unchecked_ref())
      .unwrap();
    self
      .callbacks
      .insert(ev_name.to_string(), Arc::new(cb));
  }

  pub fn bundle(&mut self, b:Bundle) {
    self.fuse_box.bundle(b);
  }

  pub fn attribute(&mut self, name: &str, init: &str, mut rx: InstantReceiver<String>) {
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

  pub fn text(&mut self, init: &str, mut rx: InstantReceiver<String>) {
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

  pub fn style(&mut self, s: &str, init: &str, mut rx: InstantReceiver<String>) {
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

  pub fn with(&mut self, init: Gizmo, mut rx: InstantReceiver<GizmoBuilder>) {
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

  //pub fn run(self) -> Result<Mogwai, JsValue> {
  //  trace!("Running gizmo {}...", self.name);
  //  trace!("  with {} bundles in its fusebox", self.fuse_box.len());

  //  let mut gizmo = self;

  //  body()
  //    .append_child(gizmo.html_element_ref())?;

  //  Ok(
  //    Mogwai(
  //      Closure::once(move || {
  //        body()
  //          .remove_child(gizmo.html_element_ref())
  //          .expect("Could not remove gizmo");
  //        gizmo.callbacks = HashMap::new();
  //        gizmo.sub_gizmos = vec![];
  //      })
  //    )
  //  )
  //}

  pub fn run(self) -> Result<(), JsValue> {
    trace!("Running gizmo {}...", self.name);
    trace!("  with {} bundles in its fusebox", self.fuse_box.len());

    body()
      .append_child(self.html_element_ref())
      .unwrap();

    let gizmo = RefCell::new(self);

    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() =
      Some(Closure::wrap(Box::new(move || {
        gizmo.borrow_mut().maintain();
        request_animation_frame(f.borrow().as_ref().unwrap());
      }) as Box<dyn Fn()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
    Ok(())
  }
}


#[wasm_bindgen]
pub struct Mogwai(Closure<FnMut()>);


fn window() -> web_sys::Window {
  web_sys::window()
    .expect("no global `window` exists")
}

fn document() -> web_sys::Document {
  window()
    .document()
    .expect("no global `document` exists")
}

fn body() -> web_sys::HtmlElement {
  document()
    .body()
    .expect("document does not have a body")
}

fn request_animation_frame(f: &Closure<dyn Fn()>) {
  window()
    .request_animation_frame(f.as_ref().unchecked_ref())
    .expect("should register `requestAnimationFrame` OK");
}

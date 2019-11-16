use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Event, EventTarget, HtmlElement, Node, Text};

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
  options: Vec<GizmoRxOption>,
  pub sub_gizmos: Vec<Gizmo>,
  pub fuse_box: FuseBox,
  is_dirty: Arc<Mutex<bool>>
}


impl Gizmo {
  pub fn new(html_element: HtmlElement) -> Gizmo {
    Gizmo {
      name: "unknown".into(),
      html_element,
      callbacks: HashMap::new(),
      options: vec![],
      sub_gizmos: vec![],
      fuse_box: FuseBox::new(),
      is_dirty: Arc::new(Mutex::new(false))
    }
  }

  /// Sends an event into the given transmitter when the given dom event happens.
  pub fn tx_on(&mut self, ev_name: &str, tx: Transmitter<()>) {
    let target:&EventTarget =
      self
      .html_element
      .dyn_ref()
      .expect("Could not get element EventTarget");

    let mut tx = tx;
    let name = self.name.clone();
    let is_dirty = self.is_dirty.clone();
    let cb =
      Closure::wrap(Box::new(move |_ev:Event| {
        trace!("{} - an event happened!", name);
        // TODO: Do something with the js event
        // push the value into the sender
        tx.push(());
        let mut dirt =
          is_dirty
          .try_lock()
          .expect("Could not try_lock in js callback");
        *dirt = true;
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

  pub fn option(&mut self, option: GizmoRxOption) {
    self.options.push(option);
  }

  pub fn attribute(&mut self, name: &str, init: &str, rx: Receiver<String>) {
    self
      .html_element
      .set_attribute(name, init)
      .expect("Could not set attribute");
    self.option(GizmoRxOption::Attribute(name.to_string(), init.into(), rx))
  }

  pub fn text(&mut self, init: &str, ds: Receiver<String>) {
    let text:Text =
      Text::new_with_data(init)
      .unwrap();
    self
      .html_element
      .clone()
      .dyn_into::<Node>()
      .expect("Could not convert gizmo element into a node")
      .append_child(text.as_ref())
      .expect("Could not add text node to gizmo element");
    self.option(GizmoRxOption::Text(text, init.into(), ds))
  }

  pub fn style(&mut self, s: &str, init: &str, rx: Receiver<String>) {
    self
      .html_element
      .dyn_ref::<HtmlElement>()
      .expect("Could not cast Element into HtmlElement")
      .style()
      .set_property(s, init)
      .expect("Could not set initial style property");
    self.option(GizmoRxOption::Style(s.to_string(), init.into(), rx))
  }

  pub fn with(&mut self, init: Gizmo, rx: Receiver<GizmoBuilder>) {
    self
      .html_element
      .dyn_ref::<Node>()
      .expect("Could not turn gizmo html_element into Node")
      .append_child(init.html_element_ref())
      .expect("Could not add initial child gizmo");
    self.option(GizmoRxOption::Gizmo(init, rx))
  }

  pub fn html_element_ref(&self) -> &HtmlElement {
    &self.html_element
  }

  fn needs_update(&self) -> bool {
    *self
      .is_dirty
      .try_lock()
      .expect("Could not try_lock Gizmo::needs_update")
  }

  fn mark_clean(&self) {
    *self
      .is_dirty
      .try_lock()
      .expect("Could not try_lock Gizmo::mark_clean")
      = false;
  }

  fn update(&mut self) {
    trace!("{} - running fuse_box...", self.name);
    self.fuse_box.run();

    trace!("{} - updating gizmo with rx...", self.name);
    let el:&Element =
      self
      .html_element
      .as_ref();
    let html_el:&HtmlElement =
      &self.html_element;
    self
      .options
      .iter_mut()
      .for_each(|option: &mut GizmoRxOption| {
        use GizmoRxOption::*;
        match option {
          Attribute(name, ref mut val, ref mut rx) => {
            rx
              .read()
              .last()
              .into_iter()
              .for_each(|s:&String| {
                trace!("  attribute {:?}", s);
                el.set_attribute(&name, &s)
                  .expect(&format!("Could not update dynamic attribute {:?} to {:?}", name, s));
                *val = s.clone();
              });
          }
          Style(name, ref mut val, rx) => {
            rx
              .read()
              .last()
              .into_iter()
              .for_each(|s| {
                trace!("  style {:?}", s);
                html_el
                  .style()
                  .set_property(&name, &s)
                  .expect(&format!("Could not update dynamic style {:?} to {:?}", name, s));
                *val = s.clone();
              });
          }
          Text(text, ref mut val, rx) => {
            rx
              .read()
              .last()
              .into_iter()
              .for_each(|s| {
                trace!("  text {:?}", s);
                text.set_data(&s);
                *val = s.clone();
              });
          }
          Gizmo(ref mut prev_gizmo, rx) => {
            rx
              .read()
              .last()
              .into_iter()
              .for_each(|gizmo_builder:&GizmoBuilder| {
                trace!("  gizmo");
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
                el.dyn_ref::<Node>()
                  .expect("Could not cast gizmo element into node")
                  .replace_child(new_node, prev_node)
                  .expect("Could not replace old gizmo with new gizmo");

                *prev_gizmo = gizmo.clone();
              })
          }
        }
      });
  }

  fn run_gizmos(&mut self, is_dirty: bool) {
    self
      .sub_gizmos
      .iter_mut()
      .for_each(|gizmo| {
        if is_dirty {
          *gizmo
            .is_dirty
            .try_lock()
            .expect("Could not try_lock Gizmo::run_gizmos")
            = true;
        }
        gizmo.maintain()
      });
  }

  pub fn maintain(&mut self) {
    let is_dirty =
      self.needs_update();
    if is_dirty {
      self.mark_clean();
      self.update();
    }
    self.run_gizmos(is_dirty);
  }

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

//! A widget.
use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::Arc;
use wasm_bindgen::closure::Closure;
use web_sys::{HtmlElement, Node, Text};

use super::prelude::*;
use super::txrx::hand_clone;
pub use super::utils::*;
pub use web_sys::{EventTarget, HtmlInputElement};
pub use wasm_bindgen::{JsCast, JsValue};


/// A bundled network of html elements, callback closures and receivers.
pub struct Gizmo {
  pub name: String,
  pub html_element: HtmlElement,
  callbacks: HashMap<String, Arc<Closure<dyn FnMut(JsValue)>>>,
  window_callbacks: HashMap<String, Arc<Closure<dyn FnMut(JsValue)>>>,
  document_callbacks: HashMap<String, Arc<Closure<dyn FnMut(JsValue)>>>,
  opt_string_rxs: Vec<Receiver<Option<String>>>,
  string_rxs: Vec<Receiver<String>>,
  bool_rxs: Vec<Receiver<bool>>,
  gizmo_rxs: Vec<Receiver<Vec<GizmoBuilder>>>,
  pub static_gizmos: Vec<Gizmo>,
}


impl Clone for Gizmo {
  fn clone(&self) -> Self {
    Gizmo {
      name: self.name.clone(),
      html_element: self.html_element.clone(),
      callbacks: self.callbacks.clone(),
      window_callbacks: self.window_callbacks.clone(),
      document_callbacks: self.document_callbacks.clone(),
      opt_string_rxs: self.opt_string_rxs.iter().map(|rx| hand_clone(rx)).collect(),
      string_rxs: self.string_rxs.iter().map(|rx| hand_clone(rx)).collect(),
      bool_rxs: self.bool_rxs.iter().map(|rx| hand_clone(rx)).collect(),
      gizmo_rxs: self.gizmo_rxs.iter().map(|rx| hand_clone(rx)).collect(),
      static_gizmos: self.static_gizmos.clone()
    }
  }
}

impl Gizmo {
  /// Create a new `Gizma` from an `HtmlElement`.
  pub fn new(html_element: HtmlElement) -> Gizmo {
    Gizmo {
      name: "unknown".into(),
      html_element,
      callbacks: HashMap::new(),
      window_callbacks: HashMap::new(),
      document_callbacks: HashMap::new(),
      opt_string_rxs: vec![],
      string_rxs: vec![],
      gizmo_rxs: vec![],
      bool_rxs: vec![],
      static_gizmos: vec![],
    }
  }

  fn add_event(
    &mut self,
    ev_name: &str,
    target: &EventTarget,
    tx: Transmitter<Event>
  ) -> Arc<Closure<dyn FnMut(JsValue)>> {
    let cb =
      Closure::wrap(Box::new(move |val:JsValue| {
        let ev =
          val
          .dyn_into()
          .expect("Callback was not an event!");
        tx.send(&ev);
      }) as Box<dyn FnMut(JsValue)>);
    target
      .add_event_listener_with_callback(ev_name, cb.as_ref().unchecked_ref())
      .unwrap();
    Arc::new(cb)
  }

  /// Sends an event into the given transmitter when the given dom event happens.
  pub fn tx_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
    let target =
      self
      .html_element
      .clone()
      .dyn_into::<EventTarget>()
      .expect("Could not get element EventTarget");
    let cb = self.add_event(ev_name, &target, tx);
    self
      .callbacks
      .insert(ev_name.to_string(), cb);
  }


  /// Sends an event into the given transmitter when the given dom event happens.
  pub fn window_tx_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
    let target =
      utils::window()
      .dyn_into::<EventTarget>()
      .expect("Could not get window EventTarget");
    let cb = self.add_event(ev_name, &target, tx);
    self
      .window_callbacks
      .insert(ev_name.to_string(), cb);
  }

  /// Sends an event into the given transmitter when the given dom event happens.
  pub fn document_tx_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
    let target =
      utils::document()
      .dyn_into::<EventTarget>()
      .expect("Could not get window EventTarget");
    let cb = self.add_event(ev_name, &target, tx);
    self
      .document_callbacks
      .insert(ev_name.to_string(), cb);
  }

  /// Add a dynamic attribute.
  pub fn attribute(&mut self, name: &str, init: Option<String>, rx: Receiver<Option<String>>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.opt_string_rxs.push(hand_clone(&rx));

    if let Some(init) = init {
      self
        .html_element
        .set_attribute(name, &init)
        .expect("Could not set attribute");
    }

    let el = self.html_element.clone();
    let name = name.to_string();

    rx.respond(move |s| {
      if let Some(s) = s {
        el.set_attribute(&name, s)
          .expect("Could not set attribute");
      } else {
        el.remove_attribute(&name)
          .expect("Could not remove attribute");
      }
    });
  }

  /// Add a dynamic boolean attribute.
  pub fn boolean_attribute(&mut self, name: &str, init: bool, rx: Receiver<bool>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.bool_rxs.push(hand_clone(&rx));

    if init {
      self
        .html_element
        .set_attribute(name, "")
        .expect("Could not set attribute");
    }

    let el = self.html_element.clone();
    let name = name.to_string();

    rx.respond(move |b| {
      if *b {
        el.set_attribute(&name, "")
          .expect("Could not set boolean attribute");
      } else {
        el.remove_attribute(&name)
          .expect("Could not remove boolean attribute")
      }
    });
  }

  /// Add a dynamic text node.
  pub fn text(&mut self, init: &str, rx: Receiver<String>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.string_rxs.push(hand_clone(&rx));

    let text:Text =
      Text::new_with_data(init)
      .unwrap();
    self
      .html_element
      .dyn_ref::<Node>()
      .expect("Could not convert gizmo element into a node")
      .append_child(text.as_ref())
      .expect("Could not add text node to gizmo element");
    rx.respond(move |s| {
      text.set_data(s);
    });
  }

  /// Add a dynamic style.
  pub fn style(&mut self, s: &str, init: &str, rx: Receiver<String>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.string_rxs.push(hand_clone(&rx));

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

    rx.respond(move |s| {
      style
        .set_property(&name, s)
        .expect("Could not set style");
    });
  }

  /// Add a dynamic value. This should only be used on gizmos with an
  /// HtmlInputElement.
  pub fn value(&mut self, init: &str, rx: Receiver<String>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.string_rxs.push(hand_clone(&rx));

    let opt_input =
      self
      .html_element
      .clone()
      .dyn_into::<HtmlInputElement>()
      .ok();

    if let Some(input) = opt_input {
      input.set_value(init);

      rx.respond(move |val:&String| {
        input.set_value(val);
      });
    } else {
      warn!("Tried to set dynamic value on a gizmo that is not an input");
    }
  }

  /// Add a dynamic list of sub-gizmos.
  pub fn gizmos(&mut self, init: Vec<Gizmo>, rx: Receiver<Vec<GizmoBuilder>>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.gizmo_rxs.push(hand_clone(&rx));

    let mut prev_gizmos = init;
    let node =
      self
      .html_element
      .clone()
      .dyn_into::<Node>()
      .expect("Could not turn gizmo html_element into Node");
    prev_gizmos
      .iter()
      .for_each(|gizmo:&Gizmo| {
        node
          .append_child(gizmo.html_element_ref())
          .expect("Could not add initial child gizmo");
      });
    rx.respond(move |gizmo_builders: &Vec<GizmoBuilder>| {
      // Build the new gizmos
      let gizmos:Vec<Gizmo> =
        gizmo_builders
        .into_iter()
        .map(|b| b.clone().build().expect("Could not build dynamic gizmos"))
        .collect();

      let max_gizmos_len =
        usize::max(gizmos.len(), prev_gizmos.len());

      for i in 0..max_gizmos_len {
        let previous =
          prev_gizmos.get(i);
        let new =
          gizmos.get(i);

        if let Some(prev) = previous {
          if let Some(new) = new {
            // Replace them

            node
              .replace_child(
                new
                  .html_element
                  .dyn_ref()
                  .unwrap(),
                prev
                  .html_element
                  .dyn_ref()
                  .unwrap()
              )
              .unwrap();
          } else {
            node
              .remove_child(
                prev
                  .html_element
                  .dyn_ref()
                  .unwrap()
              )
              .unwrap();
          }
        } else {
          let new_node =
            new
            .unwrap()
            .html_element
            .dyn_ref()
            .unwrap();
          node
            .append_child(new_node)
            .unwrap();
        }
      }

      prev_gizmos = gizmos;
    });
  }

  pub fn html_element_ref(&self) -> &HtmlElement {
    &self.html_element
  }

  pub fn maintain(&mut self) {}

  /// Append this gizmo to a parent `HtmlElement`.
  pub fn append_to(&self, parent: &HtmlElement) {
    parent
      .append_child(self.html_element_ref())
      .map_err(|_| "could not append gizmo to document body".to_string())
      .unwrap();
  }

  /// Run this gizmo in a parent container forever, never dropping it.
  pub fn run_in_container(self, container:HtmlElement) -> Result<(), JsValue> {
    if cfg!(target_arch = "wasm32") {
      self.append_to(&container);
      let gizmo = RefCell::new(self);
      timeout(1000, move || {
        gizmo.borrow_mut().maintain();
        true
      });
      Ok(())
    } else {
      Err("running gizmos is only supported on wasm".into())
    }
  }

  /// Run this gizmo in the document body forever, never dropping it.
  pub fn run(self) -> Result<(), JsValue> {
    if cfg!(target_arch = "wasm32") {
      self
        .run_in_container(body())
    } else {
      Err("running gizmos is only supported on wasm".into())
    }
  }
}

/// Gizmo's Drop implementation insures that responders no longer attempt to
/// update the gizmo. It also removes its html_element from the DOM.
impl Drop for Gizmo {
  fn drop(&mut self) {
    let node =
      self
      .html_element
      .dyn_ref::<Node>()
      .unwrap();

    node
      .parent_node()
      .iter()
      .for_each(|parent| {
        parent
          .remove_child(&node)
          .unwrap();
      });

    self
      .opt_string_rxs
      .iter_mut()
      .for_each(|rx| rx.drop_responder());

    self
      .string_rxs
      .iter_mut()
      .for_each(|rx| rx.drop_responder());

    self
      .bool_rxs
      .iter_mut()
      .for_each(|rx| rx.drop_responder());

    self
      .gizmo_rxs
      .iter_mut()
      .for_each(|rx| rx.drop_responder());
  }
}

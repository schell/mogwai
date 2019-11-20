use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::Arc;
use wasm_bindgen::closure::Closure;
use web_sys::{HtmlElement, Node, Text};

use super::prelude::*;
pub use super::utils::*;
pub use web_sys::EventTarget;
pub use wasm_bindgen::{JsCast, JsValue};


/// A gizmo is a compiled network of html tags, callback closures and message
/// responders (in the form of receivers).
#[derive(Clone)]
pub struct Gizmo {
  pub name: String,
  html_element: HtmlElement,
  callbacks: HashMap<String, Arc<Closure<dyn FnMut(JsValue)>>>,
  opt_string_rxs: Vec<Receiver<Option<String>>>,
  string_rxs: Vec<Receiver<String>>,
  bool_rxs: Vec<Receiver<bool>>,
  gizmo_rxs: Vec<Receiver<Vec<GizmoBuilder>>>,
  pub static_gizmos: Vec<Gizmo>,
}

impl Gizmo {
  pub fn new(html_element: HtmlElement) -> Gizmo {
    Gizmo {
      name: "unknown".into(),
      html_element,
      callbacks: HashMap::new(),
      opt_string_rxs: vec![],
      string_rxs: vec![],
      gizmo_rxs: vec![],
      bool_rxs: vec![],
      static_gizmos: vec![],
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
        let ev =
          val
          .dyn_into()
          .expect("Callback was not an event!");
        tx.send(&ev);
      }) as Box<dyn FnMut((JsValue))>);
    target
      .add_event_listener_with_callback(ev_name, cb.as_ref().unchecked_ref())
      .unwrap();
    self
      .callbacks
      .insert(ev_name.to_string(), Arc::new(cb));
  }

  pub fn attribute(&mut self, name: &str, init: Option<String>, mut rx: Receiver<Option<String>>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.opt_string_rxs.push(rx.clone());

    if let Some(init) = init {
      self
        .html_element
        .set_attribute(name, &init)
        .expect("Could not set attribute");
    }

    let el = self.html_element.clone();
    let name = name.to_string();

    rx.set_responder(move |s| {
      if let Some(s) = s {
        el.set_attribute(&name, s)
          .expect("Could not set attribute");
      } else {
        el.remove_attribute(&name)
          .expect("Could not remove attribute");
      }
    });
  }

  pub fn boolean_attribute(&mut self, name: &str, init: bool, mut rx: Receiver<bool>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.bool_rxs.push(rx.clone());

    if init {
      self
        .html_element
        .set_attribute(name, "")
        .expect("Could not set attribute");
    }

    let el = self.html_element.clone();
    let name = name.to_string();

    rx.set_responder(move |b| {
      if *b {
        el.set_attribute(&name, "")
          .expect("Could not set boolean attribute");
      } else {
        el.remove_attribute(&name)
          .expect("Could not remove boolean attribute")
      }
    });
  }

  pub fn text(&mut self, init: &str, mut rx: Receiver<String>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.string_rxs.push(rx.clone());

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
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.string_rxs.push(rx.clone());

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

  pub fn gizmos(&mut self, init: Vec<Gizmo>, mut rx: Receiver<Vec<GizmoBuilder>>) {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.gizmo_rxs.push(rx.clone());

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
    rx.set_responder(move |gizmo_builders: &Vec<GizmoBuilder>| {
      // Build the new gizmos
      let gizmos:Vec<Gizmo> =
        gizmo_builders
        .into_iter()
        .map(|b| b.build().expect("Could not build dynamic gizmos"))
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

  pub fn run_in_container(self, container:HtmlElement) -> Result<(), JsValue> {
    trace!("Running gizmo {}...", self.name);

    if cfg!(target_arch = "wasm32") {
      container
        .append_child(self.html_element_ref())
        .map_err(|_| "could not append gizmo to document body".to_string())?;

      let gizmo = RefCell::new(self);

      timeout(1000, move || {
        // TODO: Use the "main loop" interval to sync stats
        // ...about the gizmo graph and wirings of gizmos.
        gizmo.borrow_mut().maintain();
        true
      });

      Ok(())
    } else {
      Err("running gizmos is only supported on wasm".into())
    }

  }

  pub fn run(self) -> Result<(), JsValue> {
    trace!("Running gizmo {}...", self.name);

    if cfg!(target_arch = "wasm32") {
      self
        .run_in_container(body())
    } else {
      Err("running gizmos is only supported on wasm".into())
    }
  }
}

/// Gizmo's Drop implementation insures that responders no longer attempt to
/// update the gizmo.
impl Drop for Gizmo {
  fn drop(&mut self) {
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

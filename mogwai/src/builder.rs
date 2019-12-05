//! A gizmo builder is used to build and wire DOM elements.
//!
//! It adheres to the rust
//! [builder pattern](https://doc.rust-lang.org/1.0.0/style/ownership/builders.html)
//! and provides functions for wiring messages in and out of the DOM.
//!
//! Here is an example of using [`GizmoBuilder`], [`Transmitter<T>`] and
//! [`Receiver<T>`] to create a button that counts its own clicks:
//! ```rust,no_run
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx, rx) =
//!   txrx_fold(
//!     0,
//!     |n:&mut i32, _:&Event| -> String {
//!       *n += 1;
//!       if *n == 1 {
//!         "Clicked 1 time".to_string()
//!       } else {
//!         format!("Clicked {} times", *n)
//!       }
//!     }
//!   );
//!
//! button()
//!   .rx_text("Clicked 0 times", rx)
//!   .tx_on("click", tx)
//!   .build().unwrap()
//!   .run().unwrap()
//! ```
//! [`GizmoBuilder`]: struct.GizmoBuilder.html
//! [`Transmitter<T>`]: ../txrx/struct.Transmitter.html
//! [`Receiver<T>`]: struct.Receiver.html
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, Event, HtmlElement, HtmlInputElement, Node, window};
use std::collections::HashMap;

use super::gizmo::Gizmo;
use super::txrx::{Transmitter, Receiver, hand_clone};
use super::component::Component;

#[macro_use]
pub mod tags;

enum Continuous<T> {
  Rx(T, Receiver<T>),
  Static(T)
}


impl<T:Clone> Clone for Continuous<T> {
  fn clone(&self) -> Self {
    match self {
      Continuous::Rx(t, rx) => {
        Continuous::Rx(t.clone(), hand_clone(rx))
      }
      Continuous::Static(t) => {
        Continuous::Static(t.clone())
      }
    }
  }
}


#[derive(Clone)]
enum GizmoOption {
  Attribute(String, Continuous<Option<String>>),
  Style(String, Continuous<String>),
  Text(Continuous<String>),
  Value(Continuous<String>),
  Gizmos(Continuous<Vec<GizmoBuilder>>),
  Prebuilt(HtmlElement),
  CaptureElement(Transmitter<HtmlElement>),
  WindowEvent(String, Transmitter<Event>),
  DocumentEvent(String, Transmitter<Event>),
}

#[derive(Clone)]
enum ElementOrTag {
  Element(HtmlElement),
  Tag(String)
}

// TODO: Consider giving GizmoBuilder a tyvar.
// For example:
// * `GizmoBuilder<HtmlElement>`
// * `GizmoBuilder<HtmlInputElement>`
// * `GizmoBuilder<HtmlSVGElement>`
// The problem to solve is how to nest GizmoBuilder(s).

/// Construction and wiring for DOM elements.
/// For an extensive list of constructor functions see [`tags`].
///
/// [`tags`]: tags/index.html
#[derive(Clone)]
pub struct GizmoBuilder {
  tag: ElementOrTag,
  name: String,
  options: Vec<GizmoOption>,
  tx_events: HashMap<String, Transmitter<Event>>,
  tx_element: Option<Transmitter<HtmlElement>>
}


impl GizmoBuilder {
  /// Create a new gizmo builder with the given tag.
  /// ```rust,ignore
  /// GizmoBuilder::new("div")
  /// ```
  pub fn new(tag: &str) -> GizmoBuilder {
    GizmoBuilder {
      name: "unamed_gizmo".into(),
      tag: ElementOrTag::Tag(tag.into()),
      options: vec![],
      tx_events: HashMap::new(),
      tx_element: None
    }
  }

  /// Create a new GizmoBuilder from an existing HtmlElement.
  pub fn from_html_element(el:HtmlElement) -> GizmoBuilder {
    GizmoBuilder {
      name: "unamed_gizmo".into(),
      tag: ElementOrTag::Element(el),
      options: vec![],
      tx_events: HashMap::new(),
      tx_element: None
    }
  }

  /// Name the GizmoBuilder.
  /// This can be useful for debugging.
  pub fn named(self, s: &str) -> GizmoBuilder {
    let mut gizmo = self;
    gizmo.name = s.into();
    gizmo
  }

  fn option(self, option: GizmoOption) -> GizmoBuilder {
    let mut gizmo = self;
    gizmo.options.push(option);
    gizmo
  }

  /// When built, send the raw HtmlElement on the given transmitter.
  /// This allows you to construct component behaviors that operate on one or
  /// more HtmlElement(s) directly. For example, you may want to use
  /// `input.focus()` within the `update` function of your component. This
  /// method allows you to store the input's `HtmlElement` once it is built.
  pub fn tx_post_build(self, tx:Transmitter<HtmlElement>) -> GizmoBuilder {
    self.option(GizmoOption::CaptureElement(tx))
  }

  /// Send events of the given name into the given transmitter.
  pub fn tx_on(self, event: &str, tx: Transmitter<Event>) -> GizmoBuilder {
    let mut b = self;
    b.tx_events.insert(event.into(), tx);
    b
  }

  /// On the given window event, send an Event on the given transmitter.
  pub fn tx_on_window(self, ev: &str, tx:Transmitter<Event>) -> GizmoBuilder {
    self.option(GizmoOption::WindowEvent(ev.into(), tx))
  }

  /// On the given document event, send an Event on the given transmitter.
  pub fn tx_on_document(self, ev: &str, tx:Transmitter<Event>) -> GizmoBuilder {
    self.option(GizmoOption::DocumentEvent(ev.into(), tx))
  }

  /// Add an unchanging attribute.
  pub fn attribute(self, name: &str, value: &str) -> GizmoBuilder {
    self.option(GizmoOption::Attribute(name.to_string(), Continuous::Static(Some(value.to_string()))))
  }

  pub fn boolean_attribute(self, name: &str) -> GizmoBuilder {
    self.option(GizmoOption::Attribute(name.to_string(), Continuous::Static(Some("".into()))))
  }

  /// Add an unchanging style.
  pub fn style(self, name: &str, value: &str) -> GizmoBuilder {
    self.option(GizmoOption::Style(name.into(), Continuous::Static(value.into())))
  }

  /// Add an unchanging class.
  /// This represents all the classes for this gizmo. If you'd like to specify
  /// more than one class call this as:
  /// ```rust
  /// extern crate mogwai;
  /// use mogwai::prelude::*;
  ///
  /// let builder = GizmoBuilder::new("div");
  /// builder.class("class1 class2 class3 etc");
  /// ```
  pub fn class(self, value: &str) -> GizmoBuilder {
    self.attribute("class", value)
  }

  /// Add an unchunging text node.
  pub fn text(self, s: &str) -> GizmoBuilder {
    self.option(GizmoOption::Text(Continuous::Static(s.to_string())))
  }

  /// Add an unchanging value.
  /// NOTE: This is only for input elements.
  pub fn value(self, s: &str) -> GizmoBuilder {
    self.option(GizmoOption::Value(Continuous::Static(s.to_string())))
  }

  /// Add an unchanging child.
  pub fn with(self, g: GizmoBuilder) -> GizmoBuilder {
    self.option(GizmoOption::Gizmos(Continuous::Static(vec![g])))
  }

  /// Add many unchinging children all at once.
  pub fn with_many(self, gs: Vec<GizmoBuilder>) -> GizmoBuilder {
    gs.into_iter()
      .fold(
        self,
        |builder, sub_gizmo_builder| builder.with(sub_gizmo_builder)
      )
  }

  /// Add a pre-built web-sys HtmlElement as a child.
  pub fn with_pre_built(self, el: HtmlElement) -> GizmoBuilder {
    self.option(GizmoOption::Prebuilt(el))
  }

  /// Add a component as a child node.
  pub fn with_component<C:Component>(self, c:C) -> GizmoBuilder {
    let builder =
      c
      .into_component()
      .builder
      .unwrap();
    self.with(builder)
  }

  /// Add an attribute that changes its value every time the given receiver
  /// receives a message. If the receiver receives `None` it will respond by
  /// removing the attribute until it receives `Some(...)`.
  pub fn rx_attribute(self, name: &str, init:Option<&str>, value: Receiver<Option<String>>) -> GizmoBuilder {
    let init =
      init
      .map(|i| i.into());
    self.option(GizmoOption::Attribute(name.to_string(), Continuous::Rx(init, value)))
  }

  pub fn rx_boolean_attribute(self, name: &str, init:bool, rx: Receiver<bool>) -> GizmoBuilder {
    let to_opt = |b:&bool| -> Option<String> {
      if *b {
        Some("".into())
      } else {
        None
      }
    };
    let init = to_opt(&init);
    let rx = rx.branch_map(move |b| to_opt(b));
    self.option(GizmoOption::Attribute(name.to_string(), Continuous::Rx(init, rx)))
  }

  /// Add a changing style attribute.
  pub fn rx_style(self, name: &str, init:&str, value: Receiver<String>) -> GizmoBuilder {
    self.option(GizmoOption::Style(name.into(), Continuous::Rx(init.into(), value)))
  }

  /// Add a changing class attribute. Requires an initial value.
  pub fn rx_class(self, init:&str, rx: Receiver<String>) -> GizmoBuilder {
    self.rx_attribute("class", init.into(), rx.branch_map(|b| Some(b.into())))
  }

  /// Add a changing text node. Requires an initial value.
  pub fn rx_text(self, init: &str, s: Receiver<String>) -> GizmoBuilder {
    self.option(GizmoOption::Text(Continuous::Rx(init.into(), s)))
  }

  /// Add a changing value. Requires an initial value. The element must be
  /// an input element.
  pub fn rx_value(self, init: &str, rx: Receiver<String>) -> GizmoBuilder {
    self.option(GizmoOption::Value(Continuous::Rx(init.into(), rx)))
  }

  /// Add a changing GizmoBuilder.
  pub fn rx_with(self, init:GizmoBuilder, rx: Receiver<GizmoBuilder>) -> GizmoBuilder {
    self.rx_with_many(vec![init], rx.branch_map(|b| vec![b.clone()]))
  }

  /// Add a changing list of GizmoBuilders.
  pub fn rx_with_many(
    self,
    init:Vec<GizmoBuilder>,
    rx: Receiver<Vec<GizmoBuilder>>
  ) -> GizmoBuilder {
    self.option(GizmoOption::Gizmos(Continuous::Rx(init, rx)))
  }

  /// Build the `GizmoBuilder` into a `Gizmo`.
  pub fn build(self) -> Result<Gizmo, JsValue> {

    let document =
      window().unwrap()
      .document().unwrap();
    let html_el:HtmlElement =
      match self.tag {
        ElementOrTag::Element(el) => { el }
        ElementOrTag::Tag(tag) => {
          document
            .create_element(&tag)?
            .dyn_into()?
        }
      };
    let el:&Element =
      html_el
      .dyn_ref()
      .expect("Could not get gizmo element");
    let mut gizmo =
      Gizmo::new(html_el.clone());
    gizmo.name = self.name.clone();
    self
      .tx_events
      .iter()
      .for_each(|(name, tx)| {
        gizmo.tx_on(&name, tx.clone());
      });
    self
      .options
      .into_iter()
      .fold(
        Ok(()),
        |res, option| {
          res?;
          use Continuous::*;
          use GizmoOption::*;
          match option {
            Attribute(name, Static(value)) => {

              if let Some(value) = value {
                el.set_attribute(&name, &value)?;
              }
              Ok(())
            }
            Attribute(name, Rx(init, dynamic)) => {

              gizmo.attribute(&name, init.clone(), dynamic.branch());
              Ok(())
            }
            Style(name, Static(value)) => {

              html_el
                .style()
                .set_property(&name, &value)
            }
            Style(name, Rx(init, dynamic)) => {

              gizmo.style(&name, &init, dynamic.branch());
              Ok(())
            }
            Text(Static(value)) => {

              let text:web_sys::Text =
                web_sys::Text::new_with_data(&value)
                .unwrap();
              html_el
                .dyn_ref::<Node>()
                .unwrap()
                .append_child(text.as_ref())?;
              Ok(())
            }
            Text(Rx(init, dynamic)) => {

              gizmo.text(&init, dynamic.branch());
              Ok(())
            }
            Value(Static(value)) => {

              html_el
                .dyn_ref::<HtmlInputElement>()
                .expect("Attempted to set the value of non-input gizmo element.")
                .set_value(&value);
              Ok(())
            }
            Value(Rx(init, rx)) => {

              html_el
                .dyn_ref::<HtmlInputElement>()
                .expect("Attempted to set the value of non-input gizmo element.")
                .set_value(&init);
              gizmo.value(&init, rx.branch());
              Ok(())
            }
            Gizmos(Static(static_gizmo_builders)) => {

              let static_gizmos:Vec<_> =
                static_gizmo_builders
                .into_iter()
                .fold(
                  Ok(vec![]),
                  |res:Result<_, JsValue>, builder| {
                    let mut gizmos = res?;
                    let gizmo = builder.build()?;
                    gizmos.push(gizmo);
                    Ok(gizmos)
                  }
                )?;

              let node =
                html_el
                .dyn_ref::<Node>()
                .expect("Could not turn gizmo html_element into Node");

              static_gizmos
                .into_iter()
                .fold(
                  Ok(()),
                  |res, static_gizmo| {
                    res?;
                    node
                      .append_child(static_gizmo.html_element_ref())?;
                    gizmo.static_gizmos.push(static_gizmo);
                    Ok(())
                })
            }
            Gizmos(Rx(init_builders, rx)) => {
              let init_gizmos =
                init_builders
                .into_iter()
                .fold(
                  Ok(vec![]),
                  |res:Result<_, JsValue>, builder| {
                    let mut gizmos = res?;
                    let gizmo = builder.build()?;
                    gizmos.push(gizmo);
                    Ok(gizmos)
                  }
                )?;

              gizmo.gizmos(init_gizmos, rx.branch());
              Ok(())
            }
            Prebuilt(el) => {
              gizmo
                .html_element
                .dyn_ref::<Node>()
                .ok_or(JsValue::NULL)?
                .append_child(
                  el.dyn_ref()
                    .ok_or(JsValue::NULL)?
                )?;
              Ok(())
            }
            CaptureElement(tx_pb) => {
              tx_pb.send(&gizmo.html_element);
              Ok(())
            }
            WindowEvent(ev, tx) => {
              gizmo.window_tx_on(&ev, tx);
              Ok(())
            }
            DocumentEvent(ev, tx) => {
              gizmo.document_tx_on(&ev, tx);
              Ok(())
            }
          }
        })?;
    Ok(gizmo)
  }
}

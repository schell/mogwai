use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, Event, HtmlElement, Node, window};
use std::collections::HashMap;

use super::gizmo::Gizmo;
use super::txrx::{Transmitter, Receiver};

#[macro_use]
pub mod tags;

#[derive(Clone)]
pub enum Continuous<T> {
  Rx(T, Receiver<T>),
  Static(T)
}


#[derive(Clone)]
pub enum GizmoOption {
  Attribute(String, Continuous<Option<String>>),
  Boolean(String, Continuous<bool>),
  Style(String, Continuous<String>),
  Text(Continuous<String>),
  Gizmos(Continuous<Vec<GizmoBuilder>>)
}


#[derive(Clone)]
pub struct GizmoBuilder {
  tag: String,
  name: String,
  options: Vec<GizmoOption>,
  tx_events: HashMap<String, Transmitter<Event>>,
}


#[cfg(test)]
mod tags_test {
  use super::GizmoBuilder;
  use super::tags::*;

  #[test]
  fn pre_test() {
    let pre_builder:GizmoBuilder = pre();
    assert_eq!(pre_builder.tag, "pre".to_string());
  }
}


impl GizmoBuilder {
  pub fn new(tag: &str) -> GizmoBuilder {
    GizmoBuilder {
      name: "unamed_gizmo".into(),
      tag: tag.into(),
      options: vec![],
      tx_events: HashMap::new()
    }
  }

  pub fn named(self, s: &str) -> GizmoBuilder {
    let mut gizmo = self;
    gizmo.name = s.into();
    gizmo
  }

  pub fn option(self, option: GizmoOption) -> GizmoBuilder {
    let mut gizmo = self;
    gizmo.options.push(option);
    gizmo
  }

  /// Add an unchanging attribute.
  pub fn attribute(self, name: &str, value: &str) -> GizmoBuilder {
    self.option(GizmoOption::Attribute(name.to_string(), Continuous::Static(Some(value.to_string()))))
  }

  pub fn boolean_attribute(self, name: &str) -> GizmoBuilder {
    self.option(GizmoOption::Boolean(name.to_string(), Continuous::Static(true)))
  }

  /// Add an unchanging style.
  pub fn style(self, name: &str, value: &str) -> GizmoBuilder {
    self.option(GizmoOption::Style(name.into(), Continuous::Static(value.into())))
  }

  /// Add the unchanging class.
  /// This represents all the classes for this gizmo. If you'd like to specify
  /// more than one class call this as:
  /// ```rust
  /// builder.class("class1 class2 class3 etc");
  /// ```
  pub fn class(self, value: &str) -> GizmoBuilder {
    self.attribute("class", value)
  }

  /// Add an unchunging text node.
  pub fn text(self, s: &str) -> GizmoBuilder {
    self.option(GizmoOption::Text(Continuous::Static(s.to_string())))
  }

  /// Add an unchanging gizmo.
  pub fn with(self, g: GizmoBuilder) -> GizmoBuilder {
    self.option(GizmoOption::Gizmos(Continuous::Static(vec![g])))
  }

  /// Add many unchinging gizmos all at once.
  pub fn with_many(self, gs: Vec<GizmoBuilder>) -> GizmoBuilder {
    gs.into_iter()
      .fold(
        self,
        |builder, sub_gizmo_builder| builder.with(sub_gizmo_builder)
      )
  }

  /// Add an attribute that changes its value every time it receives a message on
  /// the given receiver. If the receiver receives `None` it will respond by
  /// removing the attribute until it receives `Some(...)`.
  pub fn rx_attribute(self, name: &str, init:Option<&str>, value: Receiver<Option<String>>) -> GizmoBuilder {
    let init =
      init
      .map(|i| i.into());
    self.option(GizmoOption::Attribute(name.to_string(), Continuous::Rx(init, value)))
  }

  pub fn rx_boolean_attribute(self, name: &str, init:bool, rx: Receiver<bool>) -> GizmoBuilder {
    self.option(GizmoOption::Boolean(name.to_string(), Continuous::Rx(init, rx)))
  }

  /// Add a changing style attribute.
  pub fn rx_style(self, name: &str, init:&str, value: Receiver<String>) -> GizmoBuilder {
    self.option(GizmoOption::Style(name.into(), Continuous::Rx(init.into(), value)))
  }

  pub fn rx_text(self, init: &str, s: Receiver<String>) -> GizmoBuilder {
    self.option(GizmoOption::Text(Continuous::Rx(init.into(), s)))
  }

  pub fn rx_with(self, init:GizmoBuilder, rx: Receiver<GizmoBuilder>) -> GizmoBuilder {
    self.option(
      GizmoOption::Gizmos(
        Continuous::Rx(
          vec![init],
          rx.branch_map(|b| Some(vec![b.clone()]))
        )
      )
    )
  }

  pub fn tx_on(self, event: &str, tx: Transmitter<Event>) -> GizmoBuilder {
    let mut b = self;
    b.tx_events.insert(event.into(), tx);
    b
  }

  pub fn build(&self) -> Result<Gizmo, JsValue> {
    trace!("building gizmo");
    let document =
      window().unwrap()
      .document().unwrap();
    let html_el:HtmlElement =
      document
      .create_element(&self.tag)?
      .dyn_into()?;
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
      .iter()
      .fold(
        Ok(()),
        |res, option| {
          res?;
          use Continuous::*;
          use GizmoOption::*;
          match option {
            Attribute(name, Static(value)) => {
              trace!("setting static attribute value on gizmo");
              if let Some(value) = value {
                el.set_attribute(&name, &value)?;
              }
              Ok(())
            }
            Attribute(name, Rx(init, dynamic)) => {
              trace!("setting dynamic attribute value on gizmo");
              gizmo.attribute(&name, init.clone(), dynamic.branch());
              Ok(())
            }
            Boolean(name, Static(should)) => {
              if *should {
                trace!("setting static boolean attribute {:?} on gizmo", name);
                el.set_attribute(&name, "")?;
              }
              Ok(())
            }
            Boolean(name, Rx(init, rx)) => {
              trace!("setting dynamic boolean attribute {:?} on gizmo", name);
              gizmo.boolean_attribute(&name, *init, rx.branch());
              Ok(())
            }
            Style(name, Static(value)) => {
              trace!("setting static style value on gizmo");
              html_el
                .style()
                .set_property(&name, &value)
            }
            Style(name, Rx(init, dynamic)) => {
              trace!("setting dynamic style {} on gizmo", init);
              gizmo.style(&name, &init, dynamic.branch());
              Ok(())
            }
            Text(Static(value)) => {
              trace!("setting static text node on gizmo");
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
              trace!("setting dynamic text node on gizmo");
              gizmo.text(&init, dynamic.branch());
              Ok(())
            }
            Gizmos(Static(static_gizmo_builders)) => {
              trace!("setting static sub-gizmos on gizmo");
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
              trace!("setting dynamic sub-gizmo on gizmo");
              gizmo.gizmos(init_gizmos, rx.branch());
              Ok(())
            }
          }
        })?;
    Ok(gizmo)
  }
}

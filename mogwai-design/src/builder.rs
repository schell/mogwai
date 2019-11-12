use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlElement, Node, Text, window};

use super::{Dynamic, Gizmo};


#[derive(Clone)]
pub enum GizmoDynamicOption {
  Attribute(String, Dynamic<String>),
  Style(String, Dynamic<String>),
  Text(Text, Dynamic<String>),
  Gizmo(Dynamic<Gizmo>)
}


pub enum Continuous<T> {
  Dynamic(Dynamic<T>),
  Static(T)
}


pub enum GizmoOption {
  Attribute(String, Continuous<String>),
  Style(String, Continuous<String>),
  Text(Continuous<String>),
  Gizmo(Continuous<Gizmo>)
}


pub struct GizmoBuilder {
  tag: String,
  options: Vec<GizmoOption>
}

pub fn div() -> GizmoBuilder {
  GizmoBuilder::new("div")
}

pub fn h1() -> GizmoBuilder {
  GizmoBuilder::new("h1")
}


pub fn button() -> GizmoBuilder {
  GizmoBuilder::new("button")
}


impl GizmoBuilder {
  fn new(tag: &str) -> GizmoBuilder {
    GizmoBuilder {
      tag: tag.into(),
      options: vec![]
    }
  }

  pub fn option(self, option: GizmoOption) -> GizmoBuilder {
    let mut gizmo = self;
    gizmo.options.push(option);
    gizmo
  }


  pub fn attribute(self, name: &str, value: &str) -> GizmoBuilder {
    self.option(GizmoOption::Attribute(name.to_string(), Continuous::Static(value.to_string())))
  }

  pub fn style(self, name: &str, value: &str) -> GizmoBuilder {
    self.option(GizmoOption::Style(name.into(), Continuous::Static(value.into())))
  }

  pub fn text(self, s: &str) -> GizmoBuilder {
    self.option(GizmoOption::Text(Continuous::Static(s.to_string())))
  }

  pub fn with(self, g: Gizmo) -> GizmoBuilder {
    self.option(GizmoOption::Gizmo(Continuous::Static(g)))
  }


  pub fn attribute_dyn(self, name: &str, value: Dynamic<String>) -> GizmoBuilder {
    self.option(GizmoOption::Attribute(name.to_string(), Continuous::Dynamic(value)))
  }

  pub fn style_dyn(self, name: &str, value: Dynamic<String>) -> GizmoBuilder {
    self.option(GizmoOption::Style(name.into(), Continuous::Dynamic(value)))
  }

  pub fn text_dyn(self, s: Dynamic<String>) -> GizmoBuilder {
    self.option(GizmoOption::Text(Continuous::Dynamic(s)))
  }

  pub fn with_dyn(self, g: Dynamic<Gizmo>) -> GizmoBuilder {
    self.option(GizmoOption::Gizmo(Continuous::Dynamic(g)))
  }

  pub fn build(self) -> Result<Gizmo, JsValue> {
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
              trace!("setting static attribute value on gizmo");
              el.set_attribute(&name, &value)
            }
            Attribute(name, Dynamic(dynamic)) => {
              trace!("setting dynamic attribute value on gizmo");
              gizmo.attribute(&name, dynamic);
              Ok(())
            }
            Style(name, Static(value)) => {
              trace!("setting static style value on gizmo");
              html_el
                .style()
                .set_property(&name, &value)
            }
            Style(name, Dynamic(dynamic)) => {
              trace!("setting dynamic style value on gizmo");
              gizmo.attribute(&name, dynamic);
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
            Text(Dynamic(dynamic)) => {
              trace!("setting dynamic text node on gizmo");
              gizmo.text(dynamic);
              Ok(())
            }
            Gizmo(Static(sub_gizmo)) => {
              trace!("setting static sub-gizmo on gizmo");
              html_el
                .dyn_ref::<Node>()
                .expect("Could not turn gizmo html_element into Node")
                .append_child(sub_gizmo.html_element_ref())?;
              gizmo.sub_gizmos.push(sub_gizmo);
              Ok(())
            }
            Gizmo(Dynamic(dynamic)) => {
              trace!("setting dynamic sub-gizmo on gizmo");
              gizmo.with(dynamic);
              Ok(())
            }
          }
      })?;
    Ok(gizmo)
  }
}

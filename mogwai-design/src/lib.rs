extern crate web_sys;

use std::collections::HashMap;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;
use std::cell::RefCell;
use std::thread;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use web_sys::{EventTarget, HtmlElement};
use specs::prelude::*;


/// Things we can take advantage of:
/// *
/// * wasm updates are typically like set_property(&self, name, value) -> Result<(), JsValue>
///   (they don't mutate)
/// * clones of elements actually reference the same DOM element

#[derive(Clone)]
pub struct Event<T> {
  value: Arc<RefCell<Option<T>>>
}


impl<T: Any> Event<T> {
  pub fn new() -> Event<()> {
    panic!("Event::new")
  }

  pub fn fire(&self) {

  }

  pub fn fold_into<Y:Any, F: Fn(Y, T) -> Y>(&self, _y:Y, _f:F) -> Dynamic<Y> {
    panic!("Event::fold_into")
  }

//  /// Create an event that fires whenever the gizmo is clicked by a user.
//  pub fn on_click(&mut self, gizmo: &Gizmo) {
//    let callback =
//      Arc::new(
//        Closure::wrap(Box::new(|| {
//          event.fire();
//        }))
//      );
//
//    event.callback = Some(callback);
//    event
//  }
}


pub struct Dynamic<T> {
  value: T,
}


impl<T:Clone> Dynamic<T> {
  ///// Create a new dynamic with the given value.
  //pub fn new(t:T) -> Dynamic<T> {
  //  panic!("Dynamic::new")
  //}

  ///// Get a reference to the dynamic's current value.
  //pub fn value(&self) -> &T {
  //  &self.value
  //}

  ///// Use the given event to update this dynamic.
  //pub fn update_on(&mut self, ev: Event<T>) {
  //  panic!("Dynamic::update_on")
  //}

  ///// Return an Event that fires whenever this Dynamic is updated.
  //pub fn event(&self) -> &Event<Y> {
  //  panic!("Dynamic::event")
  //}

  ///// Update the value of the dynamic.
  //pub fn update()
}


/// A continuous value of T.
pub enum Continuous<T> {
  Static(T),
  Dynamic(Dynamic<T>)
}


//pub trait IntoContinuous<T> {
//  fn into_continuous(t: T) -> Continuous<T> {
//
//  }
//}


pub struct Gizmo {
  html_element: HtmlElement,
  world: World,
  callbacks: HashMap<String, Closure<Fn()>>
}


impl Gizmo {
  pub fn on(&mut self, ev_name: &str) -> Event<()> {
    let target:EventTarget =
      EventTarget::from(self.html_element.clone());
    let cell:Arc<RefCell<Option<()>>> =
      Arc::new(
        RefCell::new(None)
      );
    let remote =
      cell.clone();
    let cb =
      Closure::wrap(Box::new(move || {
        //println!("Callback {:?}", ev_name.to_string().clone());
        let mut opt =
          remote
          .as_ref()
          .borrow_mut();
        *opt = Some(());
      }) as Box<Fn()>);
    target
      .add_event_listener_with_callback(ev_name, cb.as_ref().unchecked_ref())
      .unwrap();
    Event {
      value: cell
    }
  }

}


//pub enum ChildNode {
//  Html(Gizmo),
//  Text(String)
//}

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
  //fn html(tag: &str) -> GizmoBuilder {
  //  GizmoBuilder {
  //    tag: tag.into(),
  //    attributes: vec![],
  //    styles: vec![],
  //    children: vec![]
  //  }
  //}

  pub fn option(self, option: GizmoOption) -> GizmoBuilder {
    let mut gizmo = self;
    gizmo.options.push(option);
    gizmo
  }
  //pub fn attribute(self, name: &str, value: Continuous<String>) -> GizmoBuilder {
  //  let mut html = self;
  //  html
  //    .attributes
  //    .push((name.into(), value.into()));
  //  html
  //}

  //pub fn style(self, name: &str, value: &str) -> GizmoBuilder {
  //  let mut html = self;
  //  html
  //    .styles
  //    .push((name.into(), value.into()));
  //  html
  //}

  //pub fn h1() -> GizmoBuilder {
  //  Self::html("h1")
  //}

  //pub fn button() -> GizmoBuilder {
  //  Self::html("button")
  //}

  //pub fn text(self, s: &str) -> GizmoBuilder {
  //  let mut html = self;
  //  html
  //    .children
  //    .push(ChildNode::Text(s.into()));
  //  html
  //}

  //pub fn id(self, s: &str) -> GizmoBuilder {
  //  let mut html = self;
  //  html
  //    .attributes
  //    .push(("id".into(), s.into()));
  //  html
  //}

  //pub fn class(self, s: &str) -> GizmoBuilder {
  //  let mut html = self;
  //  html
  //    .attributes
  //    .push(("class".into(), s.into()));
  //  html
  //}

  //pub fn with(self, child: GizmoBuilder) -> GizmoBuilder {
  //  let mut html = self;
  //  html
  //    .children
  //    .push(ChildNode::GizmoBuilder(child));
  //  html
  //}

  pub fn build(self) -> Result<Gizmo, JsValue> {
    panic!("GizmoBuilder::build")
    //let el:Element =
    //  document
    //  .create_element(&self.tag)?;
    //let js_el:&JsValue =
    //  el.as_ref();
    //let html_el:HtmlElement =
    //  HtmlElement::from(js_el.clone());
    //self
    //  .options
    //  .into_iter()
    //  .fold(
    //    Ok(()),
    //    |res, option| {
    //      match option {
    //
    //      }
    //      let _next_res = res?;
    //      el.set_attribute(&name, &val)
    //  })?;
    //self
    //  .styles
    //  .into_iter()
    //  .fold(
    //    Ok(()),
    //    |res, (name, val)| {
    //      res?;
    //      html_el
    //      .style()
    //      .set_property(&name, &val)
    //  })?;
    //self
    //  .children
    //  .into_iter()
    //  .fold(
    //    Ok(()),
    //    |res: Result<(), JsValue>, child| {
    //      res?;

    //      let _node:Node =
    //        match child {
    //          ChildNode::Text(s) => {
    //            let text =
    //              Text::new_with_data(&s)?;

    //            el.append_child(&text)?
    //          }
    //          ChildNode::GizmoBuilder(h) => {
    //            let node =
    //              h.build(document)?;
    //            el.append_child(&node)?
    //          }
    //        };
    //      Ok(())
    //  })?;
    //Ok(el)
  }

  pub fn maintain(&mut self) {

  }
}




#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn can_sub_and_update() {
  }
}

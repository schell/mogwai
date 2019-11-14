extern crate console_log;
#[macro_use]
extern crate log;
extern crate web_sys;
extern crate crossbeam;
extern crate shrev;

use std::collections::HashMap;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;
use std::cell::{Ref, RefCell};
use std::rc::Rc;
use std::thread;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Event, EventTarget, HtmlElement, Node, Text};
use crossbeam::{Receiver, Sender};


mod builder;
pub use builder::*;

mod wire;
pub use wire::*;

/// Things we can take advantage of:
/// * javascript is single threaded (wasm may not be in the future)
/// * wasm updates are typically like set_property(&self, name, value) -> Result<(), JsValue>
///   (they don't mutate)
/// * clones of elements actually reference the same DOM element

/// We need types that represent a value hole and a value filler. Then we need
/// a type that when a hole is filled, the inner type variable is erased.

//#[derive(Clone)]
//pub struct Event<T> {
//  value: Arc<RefCell<Option<T>>>
//}
//
//
//impl<T: Any + Clone> Event<T> {
//  pub fn new() -> Event<T> {
//    Event {
//      value: Arc::new(RefCell::new(None))
//    }
//  }
//
//  fn insert(&self, t:T) {
//    *self.value.borrow_mut() = Some(t);
//  }
//
//  fn take(&self) -> Option<T> {
//    self.value.borrow_mut().take()
//  }
//
//  pub fn fold_into<Y:Any + Clone, F: Fn(Y, T) -> Y + 'static>(self, init:Y, f:F) -> Wire<Y> {
//    let src =
//      self.value;
//    let acc:RefCell<Y> =
//      RefCell::new(init.clone());
//    // gen will read a value from the event's refcell and accumulate it,
//    // producing the new accumalted value
//    let gen:Arc<Box<Fn() -> Option<Y>>> =
//      Arc::new(
//        Box::new(move || {
//          src
//            .as_ref()
//            .borrow()
//            .as_ref()
//            .cloned()
//            .map(|t:T| {
//              let y:Y =
//                acc
//                .borrow()
//                .clone();
//              let new_y:Y =
//                f(y, t);
//              *acc.borrow_mut() = new_y.clone();
//              new_y
//            })
//        })
//      );
//
//    Wire{
//      value: RefCell::new(init.clone()),
//      gen
//    }
//  }
//}
//
//
//#[derive(Clone)]
//pub struct Var<A, B> {
//  gen: Arc<Box<dyn Fn(A) -> (B, Var<A, B>)>>
//}
//
//
//impl<A, B:Any + Clone> Var<A, B> {
//  /// Create a new varying value that ironically produces the input, un-varyingly
//  /// for all eternity.
//  pub fn constant<X: Into<B>>(value: X) -> Var<A, B> {
//    let b:B = value.into();
//    Var {
//      gen: Arc::new(Box::new(move |_| (b, Var::constant(b.clone()))))
//    }
//  }
//
//  /// Create a new varying value that changes over time based on the input
//  /// function.
//  fn new(f:Box<dyn Fn(A) -> (B, Var<A, B>)>) -> Var<A, B> {
//    Var {
//      gen: Arc::new(f)
//    }
//  }
//
//  fn run(self, a:A) -> (B, Var<A, B>) {
//    self
//      .gen
//      .as_ref()(a)
//  }
//
//  pub fn map<C>(self, f:) -> Var<A, C> {
//    let old = self;
//
//  }
//}

#[derive(Clone)]
pub struct Gizmo {
  html_element: HtmlElement,
  is_dirty: Arc<RefCell<bool>>,
  callbacks: HashMap<String, Arc<Closure<Fn(Event)>>>,
  options: Vec<GizmoRxOption>,
  sub_gizmos: Vec<Gizmo>,
}


impl Gizmo {
  pub fn new(html_element: HtmlElement) -> Gizmo {
    Gizmo {
      html_element,
      is_dirty: Arc::new(RefCell::new(false)),
      callbacks: HashMap::new(),
      options: vec![],
      sub_gizmos: vec![]
    }
  }

  pub fn on(&mut self, ev_name: &str, tx: Sender<Event>) {
    let target:&EventTarget =
      self
      .html_element
      .dyn_ref()
      .expect("Could not get element EventTarget");
    let dirty_marker =
      self
      .is_dirty
      .clone();
    let cb =
      Closure::wrap(Box::new(move |ev:Event| {
        trace!("an event happened!");
        // push the value into the sender
        tx.send(ev)
          .unwrap();

        // also mark the gizmo as dirty so it knows it should update its dynamic
        // content.
        *dirty_marker
          .as_ref()
          .borrow_mut() = true;
      }) as Box<Fn(Event)>);
    target
      .add_event_listener_with_callback(ev_name, cb.as_ref().unchecked_ref())
      .unwrap();
    self
      .callbacks
      .insert(ev_name.to_string(), Arc::new(cb));
  }

  pub fn option(&mut self, option: GizmoRxOption) {
    self.options.push(option);
  }

  pub fn attribute(&mut self, name: &str, init: &str, rx: Receiver<String>) {
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
    self.option(GizmoRxOption::Style(s.to_string(), init.into(), rx))
  }

  pub fn with(&mut self, init: Gizmo, rx: Receiver<Gizmo>) {
    self.option(GizmoRxOption::Gizmo(init, rx))
  }

  pub fn html_element_ref(&self) -> &HtmlElement {
    &self.html_element
  }

  fn needs_update(&self) -> bool {
    *self.is_dirty.as_ref().borrow()
  }

  fn mark_clean(&self) {
    *self.is_dirty.as_ref().borrow_mut() = false;
  }

  fn run_dynamics(&mut self) {
    trace!("updating dynamics...");
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
          Attribute(name, ref mut val, rx) => {
            rx
              .try_recv()
              .into_iter()
              .for_each(|s:String| {
                trace!("  attribute {:?}", s);
                el.set_attribute(&name, &s)
                  .expect(&format!("Could not update dynamic attribute {:?} to {:?}", name, s));
                *val = s;
              });
          }
          Style(name, ref mut val, rx) => {
            rx
              .try_recv()
              .into_iter()
              .for_each(|s| {
                trace!("  style {:?}", s);
                html_el
                  .style()
                  .set_property(&name, &s)
                  .expect(&format!("Could not update dynamic style {:?} to {:?}", name, s));
                *val = s;
              });
          }
          Text(text, ref mut val, rx) => {
            rx
              .try_recv()
              .into_iter()
              .for_each(|s| {
                trace!("  text {:?}", s);
                text.set_data(&s);
                *val = s;
              });
          }
          Gizmo(ref mut prev_gizmo, rx) => {
            rx
              .try_recv()
              .into_iter()
              .for_each(|gizmo| {
                trace!("  gizmo");
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

                *prev_gizmo = gizmo;
              })
          }
        }
      })
  }

  fn run_gizmos(&mut self) {
    self
      .sub_gizmos
      .iter_mut()
      .for_each(|gizmo| gizmo.maintain());
  }

  pub fn maintain(&mut self) {
    if self.needs_update() {
      self.mark_clean();
      self.run_dynamics();
    }
    self.run_gizmos();
  }

  pub fn run(self) -> Result<(), JsValue> {
    trace!("Running gizmo...");

    body()
      .append_child(self.html_element_ref())
      .unwrap();

    let gizmo = RefCell::new(self);

    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() =
      Some(Closure::wrap(Box::new(move || {
        trace!("maintain()");
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

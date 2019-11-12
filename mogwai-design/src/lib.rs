extern crate console_log;
#[macro_use]
extern crate log;
extern crate web_sys;

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
use web_sys::{Element, EventTarget, HtmlElement, Node, Text};
use specs::prelude::*;

mod builder;
pub use builder::*;

/// Things we can take advantage of:
/// * javascript is single threaded (wasm may not be in the future)
/// * wasm updates are typically like set_property(&self, name, value) -> Result<(), JsValue>
///   (they don't mutate)
/// * clones of elements actually reference the same DOM element

#[derive(Clone)]
pub struct Event<T> {
  value: Arc<RefCell<Option<T>>>
}


impl<T: Any + Clone> Event<T> {
  pub fn new() -> Event<T> {
    Event {
      value: Arc::new(RefCell::new(None))
    }
  }

  fn insert(&self, t:T) {
    *self.value.borrow_mut() = Some(t);
  }

  fn take(&self) -> Option<T> {
    self.value.borrow_mut().take()
  }

  pub fn fold_into<Y:Any + Clone, F: Fn(Y, T) -> Y + 'static>(self, init:Y, f:F) -> Dynamic<Y> {
    let src =
      self.value;
    let acc:RefCell<Y> =
      RefCell::new(init.clone());
    // gen will read a value from the event's refcell and accumulate it,
    // producing the new accumalted value
    let gen:Arc<Box<Fn() -> Option<Y>>> =
      Arc::new(
        Box::new(move || {
          src
            .as_ref()
            .borrow()
            .as_ref()
            .cloned()
            .map(|t:T| {
              let y:Y =
                acc
                .borrow()
                .clone();
              let new_y:Y =
                f(y, t);
              *acc.borrow_mut() = new_y.clone();
              new_y
            })
        })
      );

    Dynamic{
      value: RefCell::new(init.clone()),
      gen
    }
  }
}


#[derive(Clone)]
pub struct Dynamic<T> {
  value: RefCell<T>,
  gen: Arc<Box<Fn() -> Option<T>>>
}


impl<T:Any + Clone> Dynamic<T> {
  ///// Create a new dynamic with the given value.
  pub fn new<X: Into<T>>(value: X) -> Dynamic<T> {
    Dynamic {
      value: RefCell::new(value.into()),
      gen: Arc::new(Box::new(|| None))
    }
  }

  fn pull_value(&self) -> Option<Ref<T>> {
    let may_t:Option<T> =
      self
      .gen
      .as_ref()();
    if let Some(t) = may_t.as_ref() {
      *self.value.borrow_mut() = t.clone();
      Some(self.value.borrow())
    } else {
      None
    }
  }

  pub fn sample(&self) -> Ref<T> {
    self
      .value
      .borrow()
  }

  /// Like `sample` but possibly avoids a costly `clone` operation. This is good
  /// for sampling dynamic gizmos.
  pub fn sample_with<X, F: Fn(&T) -> X>(&self, f:F) -> X {
    let t:&T =
      &self
      .value
      .borrow();
    f(t)
  }

  pub fn map<X:Any, F: Fn(&T) -> X + 'static>(self, f:F) -> Dynamic<X> {
    let old = self;
    let prev_value:T =
      old
      .value
      .borrow()
      .clone();
    let new_value:X =
      f(&prev_value);
    let gen:Arc<Box<Fn() -> Option<X>>> =
      Arc::new(
        Box::new(move || {
          old
            .pull_value()
            .map(|t:Ref<T>| f(&t))
        })
      );
    Dynamic {
      value: RefCell::new(new_value),
      gen
    }
  }

  /// Replace this dynamic with another one.
  /// This is useful for declaring the way a dynamic gets its values after the
  /// dynamic has been added to a gizmo.
  pub fn replace_with(&mut self, d: Dynamic<T>) {
    // Really just take that dynamic's gen so the next update comes from its
    // upstream events.
    self.gen = d.gen;
  }
}


#[derive(Clone)]
pub struct Gizmo {
  html_element: HtmlElement,
  is_dirty: Arc<RefCell<bool>>,
  clear_fns: Vec<Arc<Fn () -> usize>>,
  callbacks: HashMap<String, Arc<Closure<Fn()>>>,
  options: Vec<GizmoDynamicOption>,
  sub_gizmos: Vec<Gizmo>,
}


impl Gizmo {
  pub fn new(html_element: HtmlElement) -> Gizmo {
    Gizmo {
      html_element,
      is_dirty: Arc::new(RefCell::new(false)),
      clear_fns: vec![],
      callbacks: HashMap::new(),
      options: vec![],
      sub_gizmos: vec![]
    }
  }

  pub fn on(&mut self, ev_name: &str) -> Event<()> {
    let target:&EventTarget =
      self
      .html_element
      .dyn_ref()
      .expect("Could not get element EventTarget");
    let event:Event<()> =
      Event::new();
    let remote =
      event.clone();
    let local =
      event.clone();
    self
      .clear_fns
      .push(
        Arc::new(
          move || {
            let _ =
              local.take();
            Arc::strong_count(&local.value)
          }
        )
      );
    let dirty_marker =
      self
      .is_dirty
      .clone();
    let cb =
      Closure::wrap(Box::new(move || {
        trace!("an event happened!");
        // push the value into the refcell
        remote.insert(());

        // also mark the gizmo as dirty so it knows it should update its dyns
        *dirty_marker
          .as_ref()
          .borrow_mut() = true;
      }) as Box<Fn()>);
    target
      .add_event_listener_with_callback(ev_name, cb.as_ref().unchecked_ref())
      .unwrap();
    self
      .callbacks
      .insert(ev_name.to_string(), Arc::new(cb));
    event
  }

  pub fn option(&mut self, option: GizmoDynamicOption) {
    self.options.push(option);
  }

  pub fn attribute(&mut self, name: &str, ds: Dynamic<String>) {
    self.option(GizmoDynamicOption::Attribute(name.to_string(), ds))
  }

  pub fn text(&mut self, ds: Dynamic<String>) {
    let text:Text =
      Text::new_with_data(&ds.sample())
      .unwrap();
    self
      .html_element
      .clone()
      .dyn_into::<Node>()
      .expect("Could not convert gizmo element into a node")
      .append_child(text.as_ref())
      .expect("Could not add text node to gizmo element");
    self.option(GizmoDynamicOption::Text(text, ds))
  }

  pub fn style(&mut self, s: &str, ds: Dynamic<String>) {
    self.option(GizmoDynamicOption::Style(s.to_string(), ds))
  }

  pub fn with(&mut self, dg: Dynamic<Gizmo>) {
    self.option(GizmoDynamicOption::Gizmo(dg))
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

  fn run_dynamics(&self) {
    trace!("updating dynamics...");
    let el:&Element =
      self
      .html_element
      .as_ref();
    let html_el:&HtmlElement =
      &self.html_element;
    self
      .options
      .iter()
      .for_each(|option| {
        use GizmoDynamicOption::*;
        match option {
          Attribute(name, dynamic) => {
            dynamic
              .pull_value()
              .into_iter()
              .for_each(|s| {
                trace!("  attribute {:?}", s);
                el.set_attribute(&name, &s)
                  .expect(&format!("Could not update dynamic attribute {:?} to {:?}", name, s));
              });
          }
          Style(name, dynamic) => {
            dynamic
              .pull_value()
              .into_iter()
              .for_each(|s| {
                trace!("  style {:?}", s);
                html_el
                  .style()
                  .set_property(&name, &s)
                  .expect(&format!("Could not update dynamic style {:?} to {:?}", name, s))
              });
          }
          Text(text, dynamic) => {
            dynamic
              .pull_value()
              .into_iter()
              .for_each(|s| {
                trace!("  text {:?}", s);
                text.set_data(&s)
              });
          }
          Gizmo(dynamic) => {
            let prev_gizmo:&Self =
              &dynamic.sample();
            let prev_node:&Node =
              prev_gizmo
              .html_element
              .dyn_ref()
              .expect("Could not cast old dynamic gizmo's html_element into node");
            dynamic
              .pull_value()
              .into_iter()
              .for_each(|gizmo| {
                trace!("  gizmo");
                let new_node:&Node =
                  &gizmo
                  .html_element
                  .dyn_ref()
                  .expect("Could not cast dynamic gizmo's html_element into node");
                el.dyn_ref::<Node>()
                  .expect("Could not cast gizmo element into node")
                  .replace_child(new_node, prev_node)
                  .expect("Could not replace old gizmo with new gizmo");
              })
          }
        }
      })
  }

  fn clear(&self) {
    self
      .clear_fns
      .iter()
      .for_each(|arc_f| {
        arc_f.as_ref()();
      });
  }

  fn run_gizmos(&self) {
    self
      .sub_gizmos
      .iter()
      .for_each(|gizmo| gizmo.maintain());
  }

  pub fn maintain(&self) {
    if self.needs_update() {
      self.mark_clean();
      self.run_dynamics();
      self.clear();
    }
    self.run_gizmos();
  }

  pub fn run(self) -> Result<(), JsValue> {
    trace!("Running gizmo...");

    let gizmo = self;

    body()
      .append_child(gizmo.html_element_ref())
      .unwrap();

    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() =
      Some(Closure::wrap(Box::new(move || {
        trace!("maintain()");
        gizmo.maintain();
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


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn all_cloned_events_are_linked() {
    let a:Event<()> =
      Event::new();
    let b =
      a.clone();
    let c =
      b.clone();
    assert_eq!(*a.value.borrow(), None);
    assert_eq!(*b.value.borrow(), None);
    assert_eq!(*c.value.borrow(), None);

    c.insert(());
    assert_eq!(*a.value.borrow(), Some(()));
    assert_eq!(*b.value.borrow(), Some(()));
    assert_eq!(*c.value.borrow(), Some(()));
  }

  #[test]
  fn dynamics_are_linked_to_events() {
    let ev:Event<()> =
      Event::new();

    let dynamic:Dynamic<u32> =
      ev
      .clone()
      .fold_into(0, |n, ()| n+1);

    assert!(dynamic.pull_value().is_none());
    assert_eq!(*dynamic.sample(), 0);

    ev.insert(());

    assert!(dynamic.pull_value().is_some());
    assert_eq!(*dynamic.sample(), 1);

    let _ = ev.take();

    assert!(dynamic.pull_value().is_none());
    assert_eq!(*dynamic.sample(), 1);

    ev.insert(());

    assert!(dynamic.pull_value().is_some());
    assert_eq!(*dynamic.sample(), 2);
  }

  #[test]
  fn dynamics_gen_can_be_replaced() {
    let ev:Event<()> =
      Event::new();

    // This dynamic is actually constant, and will get replaced by an accumulated
    // event.
    let mut a:Dynamic<u32> = Dynamic::new(0 as u32);

    // It doesn't matter how many times we pull, it always has no new value
    // and sampling always gives us 0.
    assert!(a.pull_value().is_none());
    assert_eq!(*a.sample(), 0);

    assert!(a.pull_value().is_none());
    assert_eq!(*a.sample(), 0);

    assert!(a.pull_value().is_none());
    assert_eq!(*a.sample(), 0);

    // Now replace the generator in `a`.
    {
      let b:Dynamic<u32> =
        ev
        .clone()
        .fold_into(0, |n, ()| n+1);

      a.replace_with(b);
    }

    assert!(a.pull_value().is_none());
    assert_eq!(*a.sample(), 0);

    ev.insert(());

    assert!(a.pull_value().is_some());
    assert_eq!(*a.sample(), 1);

    let _ = ev.take();

    assert!(a.pull_value().is_none());
    assert_eq!(*a.sample(), 1);

    ev.insert(());

    assert!(a.pull_value().is_some());
    assert_eq!(*a.sample(), 2);
  }

  #[test]
  fn cloned_dynamics_are_linked() {
    let mut a:Dynamic<i32> =
      Dynamic::new(666);

    let b:Dynamic<i32> =
      a.clone();

    let ev:Event<()> =
      Event::new();

    let c:Dynamic<i32> =
      ev
      .clone()
      .fold_into(0, |n, ()| n+1);

    a.replace_with(c);

    ev.insert(());

    assert!(b.pull_value().is_some());
    assert_eq!(*b.sample(), 1);
  }
}

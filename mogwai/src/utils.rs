use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys;

use super::gizmo::Gizmo;
use super::txrx::Transmitter;


pub fn window() -> web_sys::Window {
  web_sys::window()
    .expect("no global `window` exists")
}

pub fn document() -> web_sys::Document {
  window()
    .document()
    .expect("no global `document` exists")
}

pub fn body() -> web_sys::HtmlElement {
  document()
    .body()
    .expect("document does not have a body")
}

pub fn set_checkup_interval(millis: i32, f: &Closure<dyn FnMut()>) -> i32 {
  window()
    .set_timeout_with_callback_and_timeout_and_arguments_0(f.as_ref().unchecked_ref(), millis)
    .expect("should register `setInterval` OK")
}

pub fn timeout<F>(millis: i32, mut logic: F) -> i32
where
  F: FnMut() -> bool + 'static
{
  // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
  let f = Rc::new(RefCell::new(None));
  let g = f.clone();

  *g.borrow_mut()
    = Some(Closure::wrap(Box::new(move || {
      let should_continue = logic();
      if should_continue {
        set_checkup_interval(millis, f.borrow().as_ref().unwrap_throw());
      }
    }) as Box<dyn FnMut()>));

  let invalidate = set_checkup_interval(millis, g.borrow().as_ref().unwrap_throw());
  invalidate
}

fn req_animation_frame(f: &Closure<dyn FnMut()>) {
  window()
    .request_animation_frame(f.as_ref().unchecked_ref())
    .expect("should register `requestAnimationFrame` OK");
}

pub fn request_animation_frame<F>(mut logic: F)
where
  F: FnMut() -> bool + 'static
{
  // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
  let f = Rc::new(RefCell::new(None));
  let g = f.clone();

  *g.borrow_mut()
    = Some(Closure::wrap(Box::new(move || {
      let should_continue = logic();
      if should_continue {
        req_animation_frame(f.borrow().as_ref().unwrap_throw());
      }
    }) as Box<dyn FnMut()>));

  req_animation_frame(g.borrow().as_ref().unwrap_throw());
  return;
}

/// Insert a child element into a parant element.
pub fn nest_gizmos(parent: &Gizmo, child: &Gizmo) -> Result<(), JsValue> {
  let child =
    child
    .html_element
    .dyn_ref::<web_sys::Node>()
    .ok_or(JsValue::NULL)?;
  let _ =
    parent
    .html_element
    .dyn_ref::<web_sys::Node>()
    .ok_or(JsValue::NULL)?
    .append_child(child)?;
  Ok(())
}


pub fn add_event(
  ev_name: &str,
  target: &web_sys::EventTarget,
  tx: Transmitter<web_sys::Event>
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
    .unwrap_throw();
  Arc::new(cb)
}

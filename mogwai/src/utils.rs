use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys;


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
        set_checkup_interval(millis, f.borrow().as_ref().unwrap());
      }
    }) as Box<dyn FnMut()>));

  let invalidate = set_checkup_interval(millis, g.borrow().as_ref().unwrap());
  invalidate
}

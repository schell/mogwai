//! Helpers and utilities.
use wasm_bindgen::{closure::Closure, JsCast, JsValue, UnwrapThrowExt};
use web_sys;

use crate::view::JsDom;

thread_local! {
    pub static WINDOW: web_sys::Window = web_sys::window().unwrap_throw();
    pub static DOCUMENT: web_sys::Document = WINDOW.with(|w| w.document().unwrap_throw());
}

/// Return the DOM [`web_sys::Window`].
/// #### Panics
/// Panics when the window cannot be returned.
pub fn window() -> JsDom {
    WINDOW.with(JsDom::from_jscast)
}

/// Return the document JsDom object [`web_sys::Document`]
/// #### Panics
/// Panics on non-wasm32 or when the document cannot be returned.
pub fn document() -> JsDom {
    DOCUMENT.with(JsDom::from_jscast)
}

/// Return the body Dom object.
///
/// ## Panics
/// Panics on wasm32 if the body cannot be returned.
pub fn body() -> JsDom {
    DOCUMENT.with(|d| JsDom::from_jscast(&d.body().expect("document does not have a body")))
}

fn req_animation_frame(f: &Closure<dyn FnMut(JsValue)>) {
    WINDOW.with(|w| {
        w.request_animation_frame(f.as_ref().unchecked_ref())
            .expect("should register `requestAnimationFrame` OK")
    });
}

/// Sets a static rust closure to be called with `window.requestAnimationFrame`.
/// The given function may return whether or not this function should be
/// rescheduled. If the function returns `true` it will be rescheduled.
/// Otherwise it will not. The static rust closure takes one parameter which is
/// a timestamp representing the number of milliseconds since the application's
/// load. See <https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp>
/// for more info.
pub fn request_animation_frame<F>(mut logic: F)
where
    F: FnMut(f64) -> bool + 'static,
{
    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let f = std::rc::Rc::new(std::cell::RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move |ts_val: JsValue| {
        let ts: f64 = ts_val.as_f64().unwrap_or_else(|| 0.0);
        let should_continue = logic(ts);
        if should_continue {
            req_animation_frame(f.borrow().as_ref().unwrap_throw());
        }
    }) as Box<dyn FnMut(JsValue)>));

    req_animation_frame(g.borrow().as_ref().unwrap_throw());
}

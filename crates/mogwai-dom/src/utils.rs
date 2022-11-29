//! Helpers and utilities.
use wasm_bindgen::{closure::Closure, JsCast, JsValue, UnwrapThrowExt};
use web_sys;

use crate::view::JsDom;

/// Return the DOM [`web_sys::Window`].
/// #### Panics
/// Panics when the window cannot be returned.
pub fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
}

/// Return the document Dom object [`web_sys::Document`]
/// #### Panics
/// Panics on non-wasm32 or when the document cannot be returned.
pub fn document() -> JsDom {
    JsDom::try_from(JsValue::from(window().document().expect("no global `document` exists"))).unwrap()
}

/// Return the body Dom object.
///
/// ## Panics
/// Panics on wasm32 if the body cannot be returned.
pub fn body() -> JsDom {
    if cfg!(target_arch = "wasm32") {
        JsDom::try_from(JsValue::from(window().document().unwrap().body().expect("document does not have a body"))).unwrap()
    } else {
        JsDom::try_from(crate::ssr::SsrElement::element("body")).map_err(|_| ()).unwrap()
    }
}

fn req_animation_frame(f: &Closure<dyn FnMut(JsValue)>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

/// Sets a static rust closure to be called with `window.requestAnimationFrame`.
/// The given function may return whether or not this function should be rescheduled.
/// If the function returns `true` it will be rescheduled. Otherwise it will not.
/// The static rust closure takes one parameter which is a timestamp representing the
/// number of milliseconds since the application's load.
/// See <https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp>
/// for more info.
pub fn request_animation_frame<F>(mut logic: F)
where
    F: FnMut(f64) -> bool + 'static,
{
    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let f = std::rc::Rc::new(std::cell::RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move |ts_val:JsValue| {
        let ts:f64 = ts_val.as_f64().unwrap_or_else(|| 0.0);
        let should_continue = logic(ts);
        if should_continue {
            req_animation_frame(f.borrow().as_ref().unwrap_throw());
        }
    }) as Box<dyn FnMut(JsValue)>));

    req_animation_frame(g.borrow().as_ref().unwrap_throw());
}

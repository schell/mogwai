//! Helpers and utilities.
use wasm_bindgen::{closure::Closure, JsCast, JsValue, UnwrapThrowExt};
use web_sys;

/// Return the DOM [`web_sys::Window`].
/// #### Panics
/// Panics when the window cannot be returned.
pub fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
}

/// Return the DOM [`web_sys::Document`]
/// #### Panics
/// Panics when the document cannot be returned.
pub fn document() -> web_sys::Document {
    window().document().expect("no global `document` exists")
}

/// Return the DOM body.
/// #### Panics
/// Panics when document.body cannot be returned.
pub fn body() -> web_sys::HtmlElement {
    document().body().expect("document does not have a body")
}

/// Set a callback closure to be called in a given number of milliseconds.
/// ### Panics
/// Panics when window.setInterval is not available.
pub fn set_checkup_interval(millis: i32, f: &Closure<dyn FnMut()>) -> i32 {
    window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(f.as_ref().unchecked_ref(), millis)
        .expect("should register `setInterval` OK")
}

/// Sets a static rust closure to be called after a given amount of milliseconds.
/// The given function may return whether or not this timeout should be rescheduled.
/// If the function returns `true` it will be rescheduled. Otherwise it will not.
pub fn timeout<F>(millis: i32, mut logic: F) -> i32
where
    F: FnMut() -> bool + 'static,
{
    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let f = std::rc::Rc::new(std::cell::RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let should_continue = logic();
        if should_continue {
            set_checkup_interval(millis, f.borrow().as_ref().unwrap_throw());
        }
    }) as Box<dyn FnMut()>));

    let invalidate = set_checkup_interval(millis, g.borrow().as_ref().unwrap_throw());
    invalidate
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
/// See https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp
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

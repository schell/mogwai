//! Helpers and utilities.
#[cfg(target_arch = "wasm32")]
use js_sys::Function;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use wasm_bindgen::{closure::Closure, JsCast, JsValue, UnwrapThrowExt};
use web_sys;

use crate::prelude::{MogwaiCallback, Transmitter};

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

/// Schedule the given closure to be run as soon as possible.
///
/// On wasm32 this schedules the closure to run async at the next "frame". Any other
/// target sees the closure called immediately.
pub fn set_immediate<F>(f: F)
where
    F: FnOnce() + 'static,
{
    if cfg!(target_arch = "wasm32") {
        // `setTimeout(0, callback)` does not run the callback immediately, there is a minimum delay of ~4ms
        // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope/setTimeout#Reasons_for_delays_longer_than_specified
        // browsers do not have a native `setImmediate(callback)` function, so we have to use a hack :(
        thread_local! {
            static PENDING: RefCell<VecDeque<Box<dyn FnOnce()>>> = Default::default();
            static CALLBACK: Closure<dyn Fn()> = Closure::wrap(Box::new(on_message));
            static SCHEDULED: Cell<bool> = Cell::new(false);
            static PORT_TO_SELF: web_sys::MessagePort = {
                let channel = web_sys::MessageChannel::new().unwrap_throw();
                CALLBACK.with(|callback| {
                    channel.port2().set_onmessage(Some(callback.as_ref().unchecked_ref()));
                });
                channel.port1()
            };
        }

        fn on_message() {
            SCHEDULED.with(|scheduled| scheduled.set(false));
            PENDING.with(|pending| {
                // callbacks can (and do) schedule more callbacks;
                // to ensure that we yield to the event loop between each batch,
                // only dequeue callbacks that were scheduled before we started running this batch
                let initial_len = pending.borrow().len();
                for _ in 0..initial_len {
                    let f = pending.borrow_mut().pop_front().unwrap_throw();
                    f();
                }
            })
        }

        PENDING.with(|pending| pending.borrow_mut().push_back(Box::new(f)));
        let was_scheduled = SCHEDULED.with(|scheduled| scheduled.replace(true));
        if !was_scheduled {
            PORT_TO_SELF.with(|port| port.post_message(&JsValue::NULL).unwrap_throw());
        }
    } else {
        f()
    }
}

/// Sets a static rust closure to be called after a given amount of milliseconds.
/// The given function may return whether or not this timeout should be rescheduled.
/// If the function returns `true` it will be rescheduled. Otherwise it will not.
pub fn timeout<F>(millis: i32, mut logic: F) -> i32
where
    F: FnMut() -> bool + 'static,
{
    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let f = Rc::new(RefCell::new(None));
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

fn req_animation_frame(f: &Closure<dyn FnMut()>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

/// Sets a static rust closure to be called with `window.requestAnimationFrame`.
/// The given function may return whether or not this function should be rescheduled.
/// If the function returns `true` it will be rescheduled. Otherwise it will not.
pub fn request_animation_frame<F>(mut logic: F)
where
    F: FnMut() -> bool + 'static,
{
    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let should_continue = logic();
        if should_continue {
            req_animation_frame(f.borrow().as_ref().unwrap_throw());
        }
    }) as Box<dyn FnMut()>));

    req_animation_frame(g.borrow().as_ref().unwrap_throw());
    return;
}

/// Add an event of the given name on the given target, transmitting any triggered
/// events into the given [`Transmitter`]. Returns the wrapped JS callback.
#[cfg(not(target_arch = "wasm32"))]
pub fn add_event(
    _ev_name: &str,
    _target: &web_sys::EventTarget,
    _tx: Transmitter<web_sys::Event>,
) -> MogwaiCallback {
    MogwaiCallback {
        callback: Rc::new(Box::new(|_| {})),
    }
}
#[cfg(target_arch = "wasm32")]
pub fn add_event(
    ev_name: &str,
    target: &web_sys::EventTarget,
    tx: Transmitter<web_sys::Event>,
) -> MogwaiCallback {
    let cb = Closure::wrap(Box::new(move |val: JsValue| {
        let ev = val.unchecked_into();
        tx.send(&ev);
    }) as Box<dyn FnMut(JsValue)>);
    target
        .add_event_listener_with_callback(ev_name, cb.as_ref().unchecked_ref())
        .unwrap_throw();
    MogwaiCallback {
        callback: Rc::new(cb),
    }
}

/// Remove an event of the given name from the given target.
#[cfg(not(target_arch = "wasm32"))]
pub fn remove_event(_ev_name: &str, _target: &web_sys::EventTarget, _cb: &MogwaiCallback) {}
#[cfg(target_arch = "wasm32")]
pub fn remove_event(ev_name: &str, target: &web_sys::EventTarget, cb: &MogwaiCallback) {
    let function: &Function = cb.callback.as_ref().as_ref().unchecked_ref();
    target
        .remove_event_listener_with_callback(ev_name, function)
        .unwrap_throw();
}


struct WaitFuture {
    start: f64,
    millis: f64,
    waker: Rc<RefCell<Option<Waker>>>,
}

impl Future for WaitFuture {
    type Output = f64;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let future: &mut WaitFuture = self.get_mut();
        let now = window().performance().expect("no performance object").now();
        let elapsed = now - future.start;
        if elapsed >= future.millis {
            Poll::Ready(elapsed)
        } else {
            *future.waker.borrow_mut() = Some(ctx.waker().clone());
            Poll::Pending
        }
    }
}

/// Wait approximately the given number of milliseconds.
/// Returns a [`Future`] that yields the actual number of milliseconds waited.
pub fn wait_approximately(millis: f64) -> impl Future<Output = f64> {
    let waker: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));
    let waker2 = waker.clone();
    let start = window().performance().expect("no performance object").now();
    timeout(millis as i32, move || {
        waker2
            .borrow_mut()
            .take()
            .into_iter()
            .for_each(|waker| waker.wake());
        false
    });
    WaitFuture {
        start,
        waker,
        millis,
    }
}

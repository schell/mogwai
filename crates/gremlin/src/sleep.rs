use futures::Future;
use std::{
    cell::RefCell,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use wasm_bindgen::{JsCast, prelude::Closure};

use crate::var::{self, Counted, Shared};

struct WaitFuture {
    start: f64,
    millis: f64,
    waker: Counted<Shared<Option<Waker>>>,
}

impl Future for WaitFuture {
    type Output = f64;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let future: &mut WaitFuture = self.get_mut();
        let now = web_sys::window()
            .unwrap()
            .performance()
            .expect("no performance object")
            .now();
        let elapsed = now - future.start;
        if elapsed >= future.millis {
            Poll::Ready(elapsed)
        } else {
            future.waker.visit_mut(|w| *w = Some(ctx.waker().clone()));
            Poll::Pending
        }
    }
}

/// Set a callback closure to be called in a given number of milliseconds.
/// ### Panics
/// Panics when window.setInterval is not available.
pub fn set_checkup_interval(millis: i32, f: &Closure<dyn FnMut()>) -> i32 {
    let fun: &js_sys::Function = f.as_ref().unchecked_ref();
    web_sys::window()
        .unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(fun, millis)
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
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let should_continue = logic();
        if should_continue {
            set_checkup_interval(millis, f.borrow().as_ref().unwrap());
        }
    }) as Box<dyn FnMut()>));

    let invalidate = set_checkup_interval(millis, g.borrow().as_ref().unwrap());
    invalidate
}

/// Wait approximately the given number of milliseconds.
/// Returns a [`Future`] that yields the actual number of milliseconds waited.
pub fn sleep_millis(millis: f64) -> impl Future<Output = f64> {
    let waker: Counted<Shared<Option<Waker>>> = var::new(None);
    let waker2 = waker.clone();
    let start = web_sys::window()
        .unwrap()
        .performance()
        .expect("no performance object")
        .now();
    timeout(millis as i32, move || {
        waker2
            .visit_mut(|w| w.take())
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

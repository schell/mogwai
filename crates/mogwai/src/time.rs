//! Wait or sleep or delay future.
use futures::Future;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue, UnwrapThrowExt};

#[cfg(not(target_arch = "wasm32"))]
lazy_static::lazy_static! {
    static ref START: std::time::Instant = std::time::Instant::now();
}

#[cfg(target_arch = "wasm32")]
/// Returns a timestamp representing the number of milliseconds (accurate
/// to within 5 microseconds if the device supports it) elapsed since an
/// arbitrary start time.
pub fn now() -> f64 {
    web_sys::window()
        .unwrap()
        .performance()
        .expect("no performance object")
        .now()
}
#[cfg(not(target_arch = "wasm32"))]
/// Returns a timestamp representing the number of milliseconds (accurate
/// to within 5 microseconds if the device supports it) elapsed since an
/// arbitrary start time.
pub fn now() -> f64 {
    START.elapsed().as_secs_f64() * 1000.0
}

struct WaitFuture {
    start: f64,
    millis: f64,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Future for WaitFuture {
    type Output = f64;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let future: &mut WaitFuture = self.get_mut();
        let t = now();
        let elapsed = t - future.start;
        if elapsed >= future.millis {
            Poll::Ready(elapsed)
        } else {
            let mut lock = future.waker.lock().unwrap();
            *lock = Some(ctx.waker().clone());
            drop(lock);

            if cfg!(not(target_arch = "wasm32")) {
                let var = future.waker.clone();
                let secs = (future.millis - elapsed) / 1000.0;
                let _ = std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs_f64(secs));
                    let mut lock = var.lock().unwrap();
                    if let Some(waker) = lock.take() {
                        waker.wake();
                    }
                });
            }

            Poll::Pending
        }
    }
}

/// Wait approximately the given number of milliseconds.
/// Returns a [`Future`] that yields the actual number of milliseconds waited.
///
// TODO: Change wait_approx to take a u64 of millis because it works better that way
pub fn wait_approx(millis: f64) -> impl Future<Output = f64> {
    let waker: Arc<Mutex<Option<Waker>>> = Default::default();
    let waker2 = waker.clone();
    let start = now();
    if cfg!(target_arch = "wasm32") {
        crate::utils::timeout(millis as i32, move || {
            waker2
                .lock()
                .unwrap()
                .take()
                .into_iter()
                .for_each(|waker| waker.wake());
            false
        });
    }
    WaitFuture {
        start,
        waker,
        millis,
    }
}

/// Wait approximately the given number of seconds.
/// Returns a [`Future`] that yields the actual number of milliseconds waited.
pub fn wait_secs(secs: f64) -> impl Future<Output = f64> {
    wait_approx(secs * 1000.0)
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
                let channel = web_sys::MessageChannel::new().unwrap();
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

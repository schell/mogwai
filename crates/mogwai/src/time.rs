//! Wait or sleep or delay future.
#[cfg(target_arch = "wasm32")]
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};
use futures_lite::{Stream, StreamExt, FutureExt};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::Closure, JsCast, JsValue, UnwrapThrowExt};

use crate::either::Either;

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

#[cfg(target_arch = "wasm32")]
/// Sets a static rust closure to be called after a given amount of milliseconds.
/// The given function may return whether or not this timeout should be rescheduled.
/// If the function returns `true` it will be rescheduled. Otherwise it will not.
pub(crate) fn timeout<F>(millis: i32, mut logic: F) -> i32
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

struct WaitFuture {
    start: f64,
    millis: u64,
    waker: Arc<std::sync::Mutex<Option<Waker>>>,
}

impl Future for WaitFuture {
    type Output = f64;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let future: &mut WaitFuture = self.get_mut();
        let t = now();
        let elapsed = t - future.start;
        if elapsed >= future.millis as f64 {
            Poll::Ready(elapsed)
        } else {
            let mut lock = future.waker.lock().unwrap();
            *lock = Some(ctx.waker().clone());
            drop(lock);

            Poll::Pending
        }
    }
}

/// Wait approximately the given number of milliseconds.
///
/// Returns a [`Future`] that yields the actual number of milliseconds waited.
pub fn wait_millis(millis: u64) -> impl Future<Output = f64> {
    let waker: Arc<std::sync::Mutex<Option<Waker>>> = Default::default();
    let start = now();

    #[cfg(target_arch = "wasm32")]
    {
        let waker2 = waker.clone();
        timeout(millis.try_into().unwrap(), move || {
            waker2
                .lock()
                .unwrap()
                .take()
                .into_iter()
                .for_each(|waker| waker.wake());
            false
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let var = waker.clone();
        let _ = std::thread::spawn(move || {
            let seconds = millis as f64 / 1000.0;
            std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
            let mut lock = var.lock().unwrap();
            if let Some(waker) = lock.take() {
                waker.wake();
            }
        });
    }

    WaitFuture {
        start,
        waker,
        millis,
    }
}

/// Wait approximately the given number of seconds.
///
/// Returns a [`Future`] that yields the actual number of milliseconds waited.
///
/// ## Note
/// This rounds the number of seconds to the closest millisecond.
pub fn wait_secs(secs: f64) -> impl Future<Output = f64> {
    let millis = secs * 1000.0;
    wait_millis(millis.round() as u64)
}

#[cfg(target_arch = "wasm32")]
/// Set a callback closure to be called in a given number of milliseconds.
/// ### Panics
/// Panics when window.setInterval is not available.
pub(crate) fn set_checkup_interval(millis: i32, f: &Closure<dyn FnMut()>) -> i32 {
    web_sys::window()
        .expect("no global window")
        .set_timeout_with_callback_and_timeout_and_arguments_0(f.as_ref().unchecked_ref(), millis)
        .expect("should register `setInterval` OK")
}

#[cfg(target_arch = "wasm32")]
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

pub async fn wait_one_frame() {
    #[cfg(target_arch = "wasm32")]
    {
        let (tx, rx) = async_channel::bounded::<()>(1);
        set_immediate(move || {
            tx.try_send(()).ok().unwrap();
        });
        // UNWRAP: safe because we control both sides of the channel and we know
        // the closure above sends a value.
        rx.recv().await.unwrap();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        futures_lite::future::yield_now().await;
    }
}

#[derive(Clone)]
pub struct Found<T> {
    pub found: T,
    pub elapsed_seconds: f64,
}

pub async fn wait_for<'a, T: 'a>(
    timeout_seconds: f64,
    mut f: impl FnMut() -> Option<T> + 'a,
) -> Result<Found<T>, f64> {
    let start = now();

    loop {
        let elapsed_seconds = (now() - start) / 1000.0;

        if elapsed_seconds >= timeout_seconds {
            return Err(elapsed_seconds);
        }

        if let Some(t) = f() {
            return Ok(Found {
                found: t,
                elapsed_seconds,
            });
        } else {
            wait_one_frame().await;
        }
    }
}

/// Wait while the given polling function returns true.
pub async fn wait_while<'a>(
    timeout_seconds: f64,
    mut f: impl FnMut() -> bool + 'a,
) -> Result<Found<()>, f64> {
    wait_for(timeout_seconds, move || if f() { None } else { Some(()) }).await
}

/// Wait until the given async-producing-function returns a value.
pub async fn wait_for_async<'a, T, A: Future<Output = Option<T>>>(
    timeout_seconds: f64,
    mut f: impl FnMut() -> A + 'a,
) -> Result<Found<T>, f64> {
    let start = now();

    loop {
        let elapsed_seconds = (now() - start) / 1000.0;

        if elapsed_seconds >= timeout_seconds {
            return Err(elapsed_seconds);
        }

        if let Some(t) = f().await {
            return Ok(Found {
                found: t,
                elapsed_seconds,
            });
        } else {
            wait_one_frame().await;
        }
    }
}

/// Run the given async-producing-function evaluating if it produces `true`,
/// if `false` wait for a given amount of time and try again N-1 times.
pub async fn repeat_times<'a, A: Future<Output = bool>>(
    timeout_seconds: f64,
    mut n_times: usize,
    mut f: impl FnMut() -> A + 'a,
) -> Result<Found<()>, f64> {
    let start = now();

    while n_times > 0 {
        n_times -= 1;

        if f().await {
            return Ok(Found {
                found: (),
                elapsed_seconds: (now() - start) / 1000.0,
            });
        } else {
            let _ = wait_secs(timeout_seconds).await;
        }
    }

    Err((now() - start) / 1000.0)
}

pub async fn wait_until_next_for<T>(
    timeout_seconds: f64,
    stream: impl Stream<Item = T> + Unpin,
) -> Result<Found<T>, f64> {
    let start = now();

    let mut stream = stream.fuse().map(Either::Left);
    let stream_next = stream.next();
    let timeout = async {
        let elapsed_millis = wait_secs(timeout_seconds).await;
        Some(Either::Right(elapsed_millis / 1000.0))
    };
    match stream_next.or(timeout).await {
        Some(Either::Left(found)) => {
            let now = now();

            let elapsed_seconds = (now - start) / 1000.0;

            Ok(Found {
                found,
                elapsed_seconds
            })
        }
        Some(Either::Right(elapsed_millis)) => {
            Err(elapsed_millis / 1000.0)
        }
        _ => Err(0.0)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod test {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn can_wait_approximately() {
        let millis_waited = wait_millis(22).await;
        assert!(millis_waited >= 21.0);
    }
}

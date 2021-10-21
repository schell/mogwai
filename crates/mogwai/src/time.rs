//! Wait or sleep or delay future.
use futures::Future;
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

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

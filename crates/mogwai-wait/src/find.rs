//! Async/await until a closure returns a value.
use mogwai::utils::{set_immediate, window};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use wasm_bindgen::UnwrapThrowExt;

#[derive(Clone)]
pub struct Found<T> {
    pub found: T,
    pub elapsed: f64,
    pub poll_count: u64,
}

pub struct FoundFuture<T> {
    op: Box<dyn Fn() -> Option<T>>,
    timeout: u32,
    poll_count: u64,
    start: f64,
}

impl<T> FoundFuture<T> {
    pub fn new<F>(timeout: u32, f: F) -> Self
    where
        F: Fn() -> Option<T> + 'static,
    {
        FoundFuture {
            op: Box::new(f),
            timeout,
            poll_count: 0,
            start: window().performance().expect("no performance object").now(),
        }
    }

    pub fn run(&self) -> Option<T> {
        (self.op)()
    }
}

impl<T> Future for FoundFuture<T> {
    type Output = Result<Found<T>, f64>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let now = window().performance().expect("no performance object").now();

        let future = self.get_mut();

        // Do some timing upkeep
        future.poll_count += 1;

        // Look for the thing
        let may_stuff: Option<T> = future.run();
        let elapsed = now - future.start;
        let elapsed_millis = elapsed.round() as u32;

        if may_stuff.is_none() && elapsed_millis <= future.timeout {
            // Set a timeout to wake this future on the next JS frame...
            let waker = Arc::new(Mutex::new(Some(ctx.waker().clone())));
            set_immediate(move || {
                let mut waker_var = waker
                    .try_lock()
                    .expect("could not acquire lock on ElementFuture waker");
                let waker: Waker = waker_var
                    .take()
                    .expect("could not unwrap stored waker on ElementFuture");
                waker.wake();
            });

            Poll::Pending
        } else if may_stuff.is_some() {
            let found = may_stuff.unwrap_throw();
            let now = window().performance().expect("no performance object").now();

            Poll::Ready(Ok(Found {
                elapsed: now - future.start,
                found,
            }))
        } else {
            let now = window().performance().expect("no performance object").now();
            Poll::Ready(Err(now - future.start))
        }
    }
}

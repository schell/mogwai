//! Instant channels. Just add water ;)
//!
//! Mostly a re-export of the [mogwai_chan] crate.
use std::{cell::RefCell, future::Future, rc::Rc};

pub use mogwai_chan::*;

#[cfg(not(target_arch = "wasm32"))]
use log::warn;

/// Provides asyncronous send and fold for mogwai's [`Transmitter`].
pub trait TransmitterAsync {
    /// Channel input.
    type Input;
    /// Wires the transmitter to the given receiver using a stateful fold function
    /// that returns an optional future. The future, if available, results in an
    /// `Option<B>`. In the case that the value of the future's result is `None`,
    /// no message will be sent to the given receiver.
    ///
    /// Lastly, a clean up function is ran at the completion of the future with its
    /// result.
    ///
    /// To aid in returning a viable future in your fold function, use
    /// `wrap_future`.
    fn wire_filter_fold_async<T, B, X, F, H>(&self, rb: &Receiver<B>, init: X, f: F, h: H)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &Self::Input) -> Option<RecvFuture<B>> + 'static,
        H: Fn(&mut T, &Option<B>) + 'static;
}

impl<A: 'static> TransmitterAsync for Transmitter<A> {
    type Input = A;
    fn wire_filter_fold_async<T, B, X, F, H>(&self, rb: &Receiver<B>, init: X, f: F, h: H)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + 'static,
        H: Fn(&mut T, &Option<B>) + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold_async(&tb, init, f, h);
    }
}

/// Provides asyncronous fold for mogwai [`Receiver`]s.
pub trait ReceiverAsync {
    /// Channel output.
    type Output;

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function that returns an optional future. The future, if
    /// returned, is executed. The future results in an `Option<B>`. In the case
    /// that the value of the future's result is `None`, no message will be sent to
    /// the transmitter.
    ///
    /// Lastly, a clean up function is ran at the completion of the future with its
    /// result.
    ///
    /// To aid in returning a viable future in your fold function, use
    /// `wrap_future`.
    fn forward_filter_fold_async<T, B, X, F, H>(self, tb: &Transmitter<B>, init: X, f: F, h: H)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &Self::Output) -> Option<RecvFuture<B>> + 'static,
        H: Fn(&mut T, &Option<B>) + 'static;
}

impl<A> ReceiverAsync for Receiver<A> {
    type Output = A;

    fn forward_filter_fold_async<T, B, X, F, H>(self, tb: &Transmitter<B>, init: X, f: F, h: H)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + 'static,
        H: Fn(&mut T, &Option<B>) + 'static,
    {
        let state = Rc::new(RefCell::new(init.into()));
        let cleanup = Rc::new(Box::new(h));
        let tb = tb.clone();
        self.respond(move |a: &A| {
            let may_async = {
                let mut block_state = state.borrow_mut();
                f(&mut block_state, a)
            };
            may_async.into_iter().for_each(|block: RecvFuture<B>| {
                let tb_clone = tb.clone();
                let state_clone = state.clone();
                let cleanup_clone = cleanup.clone();
                let future = async move {
                    let opt: Option<B> = block.await;
                    opt.iter().for_each(|b| tb_clone.send(&b));
                    let mut inner_state = state_clone.borrow_mut();
                    cleanup_clone(&mut inner_state, &opt);
                };
                wasm_bindgen_futures::spawn_local(future);
            });
        });
    }
}

//! Instant channels. Just add water ;)
//!
//! Mostly a re-export of the [mogwai_chan] crate.
use std::{cell::RefCell, rc::Rc};

pub use mogwai_chan::*;

/// Provides asyncronous send and fold for mogwai's [`Transmitter`].
pub trait TransmitterAsync {
    /// Channel input.
    type Input;

}

impl<A: 'static> TransmitterAsync for Transmitter<A> {
    type Input = A;
}

/// Provides asyncronous fold for mogwai [`Receiver`]s.
pub trait ReceiverAsync {
    /// Channel output.
    type Output;

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
}

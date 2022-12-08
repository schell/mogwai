//! Futures, streams, sinks.
//!
//! Re-exports of the futures crate, along with extensions and helper types.
use std::{
    sync::{Arc, RwLock},
    task::Poll,
};

use futures::future::Either;
pub use futures::{future, select, select_biased, stream_select, Sink, SinkExt, Stream, StreamExt};

use crate::channel::{broadcast, SinkError};

pub mod sink;
pub mod stream;

pub mod lock {
    //! Re-exports of futures::lock and async_lock.
    pub use async_lock::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard, RwLockWriteGuard};
    pub use futures::lock::*;
}

/// Adds helpful extensions to [`Either`].
pub trait EitherExt {
    /// The left item.
    type LeftItem;

    /// The right item.
    type RightItem;

    /// A type that is the result of `map_either`.
    type Mapped<F, G>: EitherExt;

    /// Return the left item, if possible.
    fn left(self) -> Option<Self::LeftItem>;

    /// Return the left item, if possible.
    fn right(self) -> Option<Self::RightItem>;

    /// Return a ref to the left item, if possible.
    fn as_left(&self) -> Option<&Self::LeftItem>;

    /// Return a ref to the left item, if possible.
    fn as_right(&self) -> Option<&Self::RightItem>;

    /// Map either the left with `f` or the right with `g`
    fn bimap<F, G>(
        self,
        f: impl FnMut(Self::LeftItem) -> F,
        g: impl FnMut(Self::RightItem) -> G,
    ) -> Self::Mapped<F, G>;

    fn map_either<T>(
        self,
        f: impl FnMut(Self::LeftItem) -> T,
        g: impl FnMut(Self::RightItem) -> T,
    ) -> T;
}

impl<A, B> EitherExt for Either<A, B> {
    type LeftItem = A;
    type RightItem = B;
    type Mapped<F, G> = Either<F, G>;

    fn left(self) -> Option<Self::LeftItem> {
        match self {
            Either::Left(a) => Some(a),
            Either::Right(_) => None,
        }
    }

    fn right(self) -> Option<Self::RightItem> {
        match self {
            Either::Right(b) => Some(b),
            Either::Left(_) => None,
        }
    }

    fn as_left(&self) -> Option<&Self::LeftItem> {
        match self {
            Either::Left(a) => Some(&a),
            Either::Right(_) => None,
        }
    }

    fn as_right(&self) -> Option<&Self::RightItem> {
        match self {
            Either::Left(_) => None,
            Either::Right(b) => Some(&b),
        }
    }

    fn bimap<F, G>(
        self,
        mut f: impl FnMut(Self::LeftItem) -> F,
        mut g: impl FnMut(Self::RightItem) -> G,
    ) -> Self::Mapped<F, G> {
        match self {
            Either::Left(l) => Either::Left(f(l)),
            Either::Right(r) => Either::Right(g(r)),
        }
    }

    fn map_either<T>(
        self,
        f: impl FnMut(Self::LeftItem) -> T,
        g: impl FnMut(Self::RightItem) -> T,
    ) -> T {
        self.bimap(f, g).into_inner()
    }
}

/// A captured future value, which uses `Sink` to store the result of an
/// operation.
pub struct Captured<T> {
    inner: Arc<RwLock<Option<T>>>,
    chan: broadcast::Channel<T>,
}

impl<T: Clone> Clone for Captured<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            chan: self.chan.clone(),
        }
    }
}

impl<T> Default for Captured<T> {
    fn default() -> Self {
        let mut chan = broadcast::Channel::new(1);
        chan.set_overflow(true);
        Self {
            inner: Arc::new(RwLock::new(None)),
            chan,
        }
    }
}

impl<T: Clone> Sink<T> for Captured<T> {
    type Error = SinkError;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let lock = self.inner.read().unwrap();
        match lock.as_ref() {
            Some(_) => Poll::Ready(Err(SinkError::Closed)),
            None => Poll::Ready(Ok(())),
        }
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        let mut lock = self.inner.write().unwrap();
        *lock = Some(item.clone());
        let sender = self.chan.sender();
        sender
            .inner
            .try_broadcast(item)
            .map_err(|_| SinkError::Closed)
            .map(|_| ())
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let lock = self.inner.read().unwrap();
        match lock.as_ref() {
            Some(_) => Poll::Ready(Ok(())),
            None => Poll::Pending,
        }
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let lock = self.inner.read().unwrap();
        match lock.as_ref() {
            Some(_) => Poll::Ready(Ok(())),
            None => Poll::Pending,
        }
    }
}

impl<T: Clone> Captured<T> {
    /// Return a sink.
    pub fn sink(&self) -> impl Sink<T, Error = SinkError> {
        self.clone()
    }

    /// Gives the current value syncronously, if possible.
    pub fn current(&self) -> Option<T> {
        let lock = self.inner.read().unwrap();
        lock.as_ref().cloned()
    }

    /// Await and return a clone of the inner `T`.
    pub async fn get(&self) -> T {
        loop {
            {
                let lock = self.inner.read().unwrap();
                if let Some(t) = lock.as_ref() {
                    return t.clone();
                }
            }

            let mut recv = self.chan.receiver();
            if let Some(t) = recv.next().await {
                return t;
            }
        }
    }
}

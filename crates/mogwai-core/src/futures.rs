//! Futures, streams, sinks.
//!
//! Re-exports of the futures crate, along with extensions and helper types.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use futures::future::Either;
pub use futures::{future, stream, Sink, SinkExt, Stream, StreamExt};

pub mod sink;

/// Adds helpful extensions to [`Either`].
pub trait EitherExt {
    /// The left item.
    type LeftItem;

    /// The right item.
    type RightItem;

    /// Return the left item, if possible.
    fn left(self) -> Option<Self::LeftItem>;

    /// Return the left item, if possible.
    fn right(self) -> Option<Self::RightItem>;
}

impl<A, B> EitherExt for Either<A, B> {
    type LeftItem = A;
    type RightItem = B;

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
}

/// A simple wrapper around an async `Sender` to help implement `Sink`.
#[derive(Clone)]
pub struct SenderSink<S, T> {
    sender: S,
    sending_msgs: Arc<Mutex<VecDeque<T>>>,
}

/// Errors returned when using [`Sink`] operations.
#[derive(Debug)]
pub enum SinkError {
    /// Receiver is closed.
    Closed,
    /// The channel is full
    Full,
}

impl<T: Clone> SenderSink<async_broadcast::Sender<T>, T> {
    fn flush_sink(&mut self) -> std::task::Poll<Result<(), SinkError>> {
        let closed = if let Some(item) = self.sending_msgs.lock().unwrap().pop_front() {
            match self.sender.try_broadcast(item) {
                Ok(_) => false,
                Err(err) => {
                    let closed = err.is_closed();
                    let item = err.into_inner();
                    self.sending_msgs.lock().unwrap().push_front(item);
                    closed
                }
            }
        } else {
            false
        };

        self.sender.set_capacity(1 + self.sender.len());

        std::task::Poll::Ready(if closed {
            Err(SinkError::Closed)
        } else {
            Ok(())
        })
    }
}

impl<T: Clone + Unpin + 'static> Sink<T> for SenderSink<async_broadcast::Sender<T>, T> {
    type Error = SinkError;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        if self.sender.len() < self.sender.capacity() {
            std::task::Poll::Ready(Ok(()))
        } else {
            std::task::Poll::Pending
        }
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        let data = self.get_mut();
        match data.sender.try_broadcast(item) {
            Ok(_) => Ok(()),
            Err(err) => match err {
                async_broadcast::TrySendError::Full(item) => {
                    let len = data.sender.len();
                    data.sender.set_capacity(1 + len);
                    data.sending_msgs.lock().unwrap().push_back(item);
                    Ok(())
                }
                async_broadcast::TrySendError::Closed(_) => Err(SinkError::Closed),
                async_broadcast::TrySendError::Inactive(_) => Ok(()),
            },
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let data = self.get_mut();
        data.flush_sink()
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let data = self.get_mut();
        let poll = data.flush_sink();
        data.sender.close();
        poll
    }
}

/// An extension trait that adds the ability for [`async_channel::Sender`] and
/// [`async_broadcast::Sender`] to ergonomically create [`Sink`]s.
pub trait IntoSenderSink<T>
where
    Self: Sized,
{
    /// Create a [`Sink`].
    fn sink(&self) -> SenderSink<Self, T>;
}

impl<T> IntoSenderSink<T> for async_broadcast::Sender<T> {
    fn sink(&self) -> SenderSink<Self, T> {
        SenderSink {
            sender: self.clone(),
            sending_msgs: Default::default(),
        }
    }
}

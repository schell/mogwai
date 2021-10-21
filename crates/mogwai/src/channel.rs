//! An async multi-producer multi-consumer channel, where each message can be received by only
//! one of all existing consumers.
//!
//! For channels where each message is received by _all_ consumers see [`broadcast`].
//!
//!
//! Mogwai uses channels to communicate between views and logic loops.
//!
//! There are two kinds of channels:
//!
//! 1. [Bounded][`bounded()`] channel with limited capacity.
//! 2. [Unbounded][`unbounded()`] channel with unlimited capacity.
//!
//! A channel is a [`Sender`] and [`Receiver`] pair. Both sides are cloneable and can be shared
//! among multiple threads.
//! When all [`Sender`]s or all [`Receiver`]s are dropped, the channel becomes closed. When a
//! channel is closed, no more messages can be sent, but remaining messages can still be received.
//!
//! Additionally, [`Sender`] can be turned into a [`Sink`] and [`Receiver`] implements [`Stream`], both of
//! which are used extensively by [`builder::ViewBuilder`] to
//! set up communication into and out of views. Please see the documentation for [`StreamExt`] and
//! [`SinkExt`] to get acquanted with the various operations available when using channels.
//!
//! # Examples
//!
//! ```
//! futures::executor::block_on(async {
//!     let (s, r) = async_channel::unbounded();
//!
//!     assert_eq!(s.send("Hello").await, Ok(()));
//!     assert_eq!(r.recv().await, Ok("Hello"));
//! });
//! ```
pub use futures::{Sink, SinkExt, Stream, StreamExt};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub use async_channel::*;

pub mod broadcast {
    //! Broadcast channels.
    pub use async_broadcast::*;

    /// Waits until the channel of the given `Sender` is empty.
    pub async fn until_empty<T>(tx: &Sender<T>) {
        while !tx.is_empty() {
            let _ = crate::time::wait_approx(0.01).await;
        }
    }
}

/// Waits until the channel of the given `Sender` is empty.
pub async fn until_empty<T>(tx: &Sender<T>) {
    while !tx.is_empty() {
        let _ = crate::time::wait_approx(0.01).await;
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

impl<T: 'static> SenderSink<async_channel::Sender<T>, T> {
    fn flush_sink(&mut self) -> Result<(), SinkError> {
        if self.sender.is_closed() {
            return Err(SinkError::Closed);
        }

        let mut msgs = self.sending_msgs.lock().unwrap();
        while let Some(item) = msgs.pop_front() {
            match self.sender.try_send(item) {
                Ok(()) => {}
                Err(err) => match err {
                    async_channel::TrySendError::Full(t) => {
                        msgs.push_front(t);
                        return Err(SinkError::Full);
                    }
                    async_channel::TrySendError::Closed(t) => {
                        msgs.push_front(t);
                        return Err(SinkError::Closed);
                    }
                },
            }
        }

        assert!(msgs.is_empty());
        Ok(())
    }
}

impl<T: Clone> SenderSink<async_broadcast::Sender<T>, T> {
    fn flush_sink(&mut self) -> std::task::Poll<Result<(), SinkError>> {
        if let Some(item) = self.sending_msgs.lock().unwrap().pop_front() {
            match self.sender.try_broadcast(item) {
                Ok(_) => {}
                Err(err) => {
                    let closed = err.is_closed();
                    let item = err.into_inner();
                    self.sending_msgs.lock().unwrap().push_front(item);
                    if closed {
                        return std::task::Poll::Ready(Err(SinkError::Closed));
                    }
                }
            }
        }

        self.sender.set_capacity(1 + self.sender.len());
        if self.sender.is_empty() {
            std::task::Poll::Ready(Ok(()))
        } else {
            std::task::Poll::Pending
        }
    }
}

impl<T: Unpin + 'static> Sink<T> for SenderSink<async_channel::Sender<T>, T> {
    type Error = SinkError;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        if self.sender.is_closed() {
            return std::task::Poll::Ready(Err(SinkError::Closed));
        }

        let cap = self.sender.capacity();

        let msgs = self.sending_msgs.lock().unwrap();
        if cap.is_none() || cap.unwrap() > msgs.len() {
            std::task::Poll::Ready(Ok(()))
        } else {
            // There are already messages in the queue
            std::task::Poll::Pending
        }
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        if self.sender.is_closed() {
            return Err(SinkError::Closed);
        }

        let mut msgs = self.sending_msgs.lock().unwrap();
        let item = {
            msgs.push_back(item);
            msgs.pop_front().unwrap()
        };

        match self.sender.try_send(item) {
            Ok(()) => Ok(()),
            Err(async_channel::TrySendError::Full(t)) => {
                msgs.push_front(t);
                Ok(())
            }
            Err(async_channel::TrySendError::Closed(t)) => {
                msgs.push_front(t);
                Err(SinkError::Closed)
            }
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let data = self.get_mut();
        match data.flush_sink() {
            Ok(()) => std::task::Poll::Ready(Ok(())),
            Err(err) => match err {
                SinkError::Closed => std::task::Poll::Ready(Err(SinkError::Closed)),
                SinkError::Full => std::task::Poll::Pending,
            },
        }
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let data = self.get_mut();
        let poll = match data.flush_sink() {
            Ok(()) => std::task::Poll::Ready(Ok(())),
            Err(err) => match err {
                SinkError::Closed => std::task::Poll::Ready(Err(SinkError::Closed)),
                SinkError::Full => std::task::Poll::Pending,
            },
        };
        data.sender.close();
        poll
    }
}

impl<T: Clone + Unpin + 'static> Sink<T> for SenderSink<async_broadcast::Sender<T>, T> {
    type Error = SinkError;

    fn poll_ready(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
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
            }
        }
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        let data = self.get_mut();
        data.flush_sink()
    }

    fn poll_close(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
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
    Self: Sized
{
    /// Create a [`Sink`].
    fn sink(&self) -> SenderSink<Self, T>;
}

impl<T> IntoSenderSink<T> for async_channel::Sender<T> {
    fn sink(&self) -> SenderSink<Self, T> {
        SenderSink {
            sender: self.clone(),
            sending_msgs: Default::default(),
        }
    }
}

impl<T> IntoSenderSink<T> for async_broadcast::Sender<T> {
    fn sink(&self) -> SenderSink<Self, T> {
        SenderSink {
            sender: self.clone(),
            sending_msgs: Default::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn channel_sinks_and_streams() {
        let (f32tx, f32rx) = bounded::<f32>(3);
        let f32stream = f32rx.map(|f| format!("{:.2}", f)).boxed();
        let (u32tx, u32rx) = bounded::<u32>(3);
        let u32stream = u32rx.map(|u| format!("{}", u)).boxed();
        let formatted = futures::stream::select_all(vec![u32stream, f32stream]);

        f32tx.send(1.5).await.unwrap();
        u32tx.send(666).await.unwrap();
        f32tx.send(2.3).await.unwrap();

        let mut strings: Vec<String> = formatted.take(3).collect::<Vec<_>>().await;
        strings.sort();

        assert_eq!(
            strings,
            vec!["1.50".to_string(), "2.30".to_string(), "666".to_string()]
        );
    }
}

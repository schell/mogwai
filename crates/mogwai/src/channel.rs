//! An asynchronous multi-producer multi-consumer channel.
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
//! Additionally, [`Sender`] implements [`Sink`] and [`Receiver`] implements [`Stream`], both of
//! which are used extensively by [`builder::ViewBuilder`] to
//! set up communication into and out of views. Please see the documentation for [`StreamExt`] and
//! [`SinkExt`] to get acquanted with the various operations available when using channels.
//!
//! # Examples
//!
//! ```
//! # futures_lite::future::block_on(async {
//! let (s, r) = async_channel::unbounded();
//!
//! assert_eq!(s.send("Hello").await, Ok(()));
//! assert_eq!(r.recv().await, Ok("Hello"));
//! # });
//! ```
pub use futures::{Sink, SinkExt, Stream, StreamExt};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// The sending side of a channel.
///
/// A simple wrapper around [`async_channel::Sender`] to help implement `Sink`.
///
/// Senders can be cloned and shared among threads. When all senders associated with a channel are
/// dropped, the channel becomes closed.
///
/// Senders implement the [`Sink`] trait.
pub struct Sender<T> {
    sender: async_channel::Sender<T>,
    sending_msgs: Arc<Mutex<VecDeque<T>>>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            sender: self.sender.clone(),
            sending_msgs: self.sending_msgs.clone(),
        }
    }
}

/// Errors returned when using [`Sink`] operations.
#[derive(Debug)]
pub enum SinkError {
    /// Receiver is closed.
    Closed,
    /// The channel is full
    Full,
}

impl<T: 'static> Sender<T> {
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

impl<T: Unpin + 'static> Sink<T> for Sender<T> {
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

/// The receiving side of a channel.
///
/// A simple wrapper around [`async_channel::Receiver`].
///
/// Receivers can be cloned and shared among threads. When all receivers associated with a channel are
/// dropped, the channel becomes closed.
///
/// Receivers implement the [`Stream`] trait.
pub struct Receiver<T>(async_channel::Receiver<T>);

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Receiver(self.0.clone())
    }
}

impl<T> Stream for Receiver<T> {
    type Item = <async_channel::Receiver<T> as Stream>::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

/// Creates a bounded channel.
///
/// The created channel has space to hold at most `cap` messages at a time.
///
/// # Panics
///
/// Capacity must be a positive number. If `cap` is zero, this function will panic.
///
/// # Examples
///
/// ```
/// # futures_lite::future::block_on(async {
/// use async_channel::{bounded, TryRecvError, TrySendError};
///
/// let (s, r) = bounded(1);
///
/// assert_eq!(s.send(10).await, Ok(()));
/// assert_eq!(s.try_send(20), Err(TrySendError::Full(20)));
///
/// assert_eq!(r.recv().await, Ok(10));
/// assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
/// # });
pub fn bounded<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = async_channel::bounded::<T>(cap);
    (
        Sender {
            sender: tx,
            sending_msgs: Default::default(),
        },
        Receiver(rx),
    )
}

/// Creates an unbounded channel.
///
/// The created channel can hold an unlimited number of messages.
///
/// # Examples
///
/// ```
/// # futures_lite::future::block_on(async {
/// use async_channel::{unbounded, TryRecvError};
///
/// let (s, r) = unbounded();
///
/// assert_eq!(s.send(10).await, Ok(()));
/// assert_eq!(s.send(20).await, Ok(()));
///
/// assert_eq!(r.recv().await, Ok(10));
/// assert_eq!(r.recv().await, Ok(20));
/// assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
/// # });
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = async_channel::unbounded::<T>();
    (
        Sender {
            sender: tx,
            sending_msgs: Default::default(),
        },
        Receiver(rx),
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn channel_sinks_and_streams() {
        let (mut f32tx, f32rx) = bounded::<f32>(3);
        let f32stream = f32rx.map(|f| format!("{:.2}", f)).boxed();
        let (mut u32tx, u32rx) = bounded::<u32>(3);
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

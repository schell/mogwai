//! Async mpmc and broadcast channels, plus extensions.

/// Errors returned when using [`futures::Sink`] operations.
#[derive(Debug)]
pub enum SinkError {
    /// Receiver is closed.
    Closed,
    /// The channel is full
    Full,
}

impl<T> From<async_broadcast::SendError<T>> for SinkError {
    fn from(_: async_broadcast::SendError<T>) -> Self {
        SinkError::Closed
    }
}

impl From<async_broadcast::RecvError> for SinkError {
    fn from(_: async_broadcast::RecvError) -> Self {
        SinkError::Closed
    }
}

impl From<futures::channel::mpsc::SendError> for SinkError {
    fn from(e: futures::channel::mpsc::SendError) -> Self {
        if e.is_disconnected() {
            SinkError::Closed
        } else {
            SinkError::Full
        }
    }
}

impl<T> From<async_broadcast::TrySendError<T>> for SinkError {
    fn from(e: async_broadcast::TrySendError<T>) -> Self {
        match e {
            async_broadcast::TrySendError::Full(_) => SinkError::Full,
            _ => SinkError::Closed,
        }
    }
}

pub mod mpsc {
    //! A multi-producer, single consumer queue.
    //!
    //! This module contains thin wrappers around types in [`futures::channel::mpsc`].
    //! See the originals for how to use the inner types.
    use futures::{
        channel::mpsc::{self, Receiver as FutReceiver, Sender as FutSender},
        Sink, SinkExt, Stream, StreamExt,
    };
    use std::task::Poll;

    use super::SinkError;

    /// Multi-producer, single consumer `Sender` that supports [`Sink`].
    #[derive(Debug)]
    pub struct Sender<T> {
        pub inner: FutSender<T>,
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }

    impl<T> Sink<T> for Sender<T> {
        type Error = SinkError;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            match data.inner.poll_ready(cx) {
                std::task::Poll::Ready(r) => Poll::Ready(match r {
                    Ok(()) => Ok(()),
                    Err(err) => Err(if err.is_disconnected() {
                        SinkError::Closed
                    } else {
                        SinkError::Full
                    }),
                }),
                std::task::Poll::Pending => Poll::Pending,
            }
        }

        fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
            let data = self.get_mut();
            data.inner.start_send(item).map_err(SinkError::from)
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            data.inner.poll_flush_unpin(cx).map_err(SinkError::from)
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            data.inner.poll_close_unpin(cx).map_err(SinkError::from)
        }
    }

    /// Multi-producer, single consumer `Receiver` that supports [`Stream`].
    ///
    /// This is a thin wrapper around [`futures::channel::mpsc::Receiver`].
    #[derive(Debug)]
    pub struct Receiver<T> {
        pub inner: FutReceiver<T>,
    }

    impl<T> Stream for Receiver<T> {
        type Item = T;

        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            let data = self.get_mut();
            data.inner.poll_next_unpin(cx)
        }
    }

    /// A bounded `Sender` and `Receiver` pair.
    pub fn bounded<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = mpsc::channel::<T>(cap);
        (Sender { inner: tx }, Receiver { inner: rx })
    }
}

pub mod broadcast {
    //! Async broadcast channel
    //!
    //! An async multi-producer multi-consumer broadcast channel, where each consumer gets a clone of every
    //! message sent on the channel. For obvious reasons, the channel can only be used to broadcast types
    //! that implement [`Clone`].
    //!
    //! A channel has the [`Sender`] and [`Receiver`] side. Both sides are cloneable and can be shared
    //! among multiple threads.
    //!
    //! When all `Sender`s or all `Receiver`s are dropped, the channel becomes closed. When a channel is
    //! closed, no more messages can be sent, but remaining messages can still be received.
    //!
    //! This is a small wrapper around [async_broadcast::Sender].

    use std::task::Poll;

    use futures::{Sink, Stream, StreamExt};

    use super::SinkError;

    /// An asynchronous broadcast sender that implements [`Sink`].
    ///
    /// This is a thin wrapper around [`async_broadcast::Sender`].
    #[derive(Clone, Debug)]
    pub struct Sender<T> {
        pub inner: async_broadcast::Sender<T>,
    }

    impl<T: Clone> Sender<T> {
        /// Broadcast a message to all linked [`Receiver`]s.
        ///
        /// If the channel was full but the send was successful, returns the oldest message
        /// in the channel.
        pub async fn broadcast(&self, item: T) -> Result<Option<T>, SinkError> {
            self.inner.broadcast(item).await.map_err(SinkError::from)
        }

        fn flush_sink(&mut self) -> std::task::Poll<Result<(), SinkError>> {
            if self.inner.is_closed() {
                return Poll::Ready(Err(SinkError::Closed));
            }
            if self.inner.capacity() >= self.inner.len() {
                return Poll::Ready(Ok(()));
            }
            Poll::Pending
        }

        /// Waits until the channel of the given `Sender` is empty.
        pub async fn until_empty(&self) {
            while !self.inner.is_empty() {
                let _ = crate::time::wait_millis(1).await;
            }
        }
    }

    impl<T: Clone> Sink<T> for Sender<T> {
        type Error = SinkError;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            if self.inner.len() < self.inner.capacity() {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }

        fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
            if self.inner.len() < self.inner.capacity() || self.inner.overflow() {
                self.inner
                    .try_broadcast(item)
                    .map(|_| ())
                    .map_err(SinkError::from)
            } else {
                Err(SinkError::Full)
            }
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            data.flush_sink()
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            let result = data.flush_sink();
            data.inner.close();
            result
        }
    }

    /// An asynchronous broadcast `Receiver`.
    ///
    /// This is a thin wrapper around [`async_broadcast::Receiver`].
    #[derive(Clone, Debug)]
    pub struct Receiver<T> {
        pub inner: async_broadcast::Receiver<T>,
    }

    impl<T: Clone> Receiver<T> {
        /// Receiver an item from the channel.
        pub async fn recv(&mut self) -> Result<T, SinkError> {
            self.inner.recv().await.map_err(SinkError::from)
        }
    }

    /// Create an asynchronous multi-producer, multi-consumer broadcast channel.
    pub fn bounded<T: Clone>(cap: usize) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = async_broadcast::broadcast::<T>(cap);
        (Sender { inner: tx }, Receiver { inner: rx })
    }

    impl<T: Clone> Stream for Receiver<T> {
        type Item = T;

        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            let data = self.get_mut();
            data.inner.poll_next_unpin(cx)
        }
    }

    /// A [`Sender`] [`Receiver`] paired together in a struct.
    #[derive(Clone)]
    pub struct Channel<T> {
        sender: async_broadcast::Sender<T>,
        receiver: async_broadcast::InactiveReceiver<T>,
    }

    impl<T> Channel<T> {
        /// Create a new broadcast channel with the given capacity.
        pub fn new(cap: usize) -> Self {
            let (sender, rx) = async_broadcast::broadcast(cap);
            Channel {
                sender,
                receiver: rx.deactivate(),
            }
        }

        /// Set the overflow of the channel.
        pub fn set_overflow(&mut self, overflow: bool) {
            self.sender.set_overflow(overflow);
        }

        /// Create a new Sender out of this channel.
        pub fn sender(&self) -> Sender<T> {
            Sender {
                inner: self.sender.clone(),
            }
        }

        /// Create a new active Receiver out of this channel.
        pub fn receiver(&self) -> Receiver<T> {
            Receiver {
                inner: self.receiver.activate_cloned(),
            }
        }
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    mod test {
        use super::*;
        use crate::futures::{SinkExt, StreamExt};

        #[test]
        fn can_sink_stream() {
            smol::block_on(async {
                let (mut tx, mut rx) = bounded::<String>(1);
                tx.send("hello".into()).await.unwrap();
                let _ = rx.next().await.unwrap();
            })
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use super::*;
    use crate::futures::*;

    #[test]
    fn channel_sinks_and_streams() {
        let (f32tx, f32rx) = broadcast::bounded::<f32>(3);
        let f32stream = f32rx.map(|f| format!("{:.2}", f)).boxed();
        let (u32tx, u32rx) = broadcast::bounded::<u32>(3);
        let u32stream = u32rx.map(|u| format!("{}", u)).boxed();
        let formatted = futures::stream::select_all(vec![u32stream, f32stream]);

        smol::block_on(async move {
            f32tx.broadcast(1.5).await.unwrap();
            u32tx.broadcast(666).await.unwrap();
            f32tx.broadcast(2.3).await.unwrap();

            let mut strings: Vec<String> = formatted.take(3).collect::<Vec<_>>().await;
            strings.sort();

            assert_eq!(
                strings,
                vec!["1.50".to_string(), "2.30".to_string(), "666".to_string()]
            );
        });
    }
}

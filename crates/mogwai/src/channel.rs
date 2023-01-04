//! Async mpmc and broadcast channels, plus extensions.
use std::num::NonZeroUsize;

/// A NonZeroUsize of one.
///
/// Use this for convenience when creating bounded channels that take a `NonZeroUsize`.
pub const ONE: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };

pub mod mpsc {
    //! A multi-producer, single consumer queue.
    //!
    //! This module contains thin wrappers around types in [async_channel].
    use crate::sink::{SendError, Sink, TrySendError};
    use std::{future::Future, marker::Send};

    pub use async_channel::{bounded, unbounded, Receiver, Sender};

    impl<Item: Send + Sync> Sink<Item> for Sender<Item> {
        fn send(
            &self,
            item: Item,
        ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + '_>> {
            Box::pin(async move { self.send(item).await.map_err(|_| SendError::Closed) })
        }

        fn try_send(&self, item: Item) -> Result<(), TrySendError> {
            match async_channel::Sender::try_send(self, item) {
                Ok(()) => Ok(()),
                Err(err) => match err {
                    async_channel::TrySendError::Full(_) => Err(TrySendError::Full),
                    async_channel::TrySendError::Closed(_) => Err(TrySendError::Closed),
                },
            }
        }
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
    use crate::{
        sink::{SendError, Sink, TrySendError},
        stream::Stream,
    };
    use std::task::Poll;

    /// An asynchronous broadcast sender that implements [`Sink`].
    ///
    /// This is a thin wrapper around [`async_broadcast::Sender`].
    #[derive(Clone, Debug)]
    pub struct Sender<T> {
        pub inner: async_broadcast::Sender<T>,
    }

    impl<Item: Clone + Send + Sync> Sink<Item> for Sender<Item> {
        fn send(
            &self,
            item: Item,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), SendError>> + Send + '_>>
        {
            Box::pin(async move {
                self.inner
                    .broadcast(item)
                    .await
                    .map(|_| ())
                    .map_err(|_| SendError::Closed)
            })
        }

        fn try_send(&self, item: Item) -> Result<(), TrySendError> {
            match self.inner.try_broadcast(item) {
                Ok(_) => Ok(()),
                Err(err) => match err {
                    async_broadcast::TrySendError::Full(_) => Err(TrySendError::Full),
                    async_broadcast::TrySendError::Closed(_) => Err(TrySendError::Closed),
                    async_broadcast::TrySendError::Inactive(_) => Ok(()),
                },
            }
        }
    }

    impl<T: Clone> Sender<T> {
        /// Broadcast a message to all linked [`Receiver`]s.
        ///
        /// If the channel was full but the send was successful, returns the oldest message
        /// in the channel.
        pub async fn broadcast(&self, item: T) -> Result<Option<T>, SendError> {
            self.inner
                .broadcast(item)
                .await
                .map_err(|_| SendError::Closed)
        }

        /// Waits until the channel of the given `Sender` is empty.
        pub async fn until_empty(&self) {
            while !self.inner.is_empty() {
                let _ = crate::time::wait_millis(1).await;
            }
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
        pub async fn recv(&mut self) -> Result<T, SendError> {
            self.inner.recv().await.map_err(|err| match err {
                async_broadcast::RecvError::Overflowed(_) => SendError::Full,
                async_broadcast::RecvError::Closed => SendError::Full,
            })
        }
    }

    /// Create an asynchronous multi-producer, multi-consumer broadcast channel.
    pub fn bounded<T: Clone>(cap: std::num::NonZeroUsize) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = async_broadcast::broadcast::<T>(cap.into());
        (Sender { inner: tx }, Receiver { inner: rx })
    }

    impl<T: Clone> Stream for Receiver<T> {
        type Item = T;

        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            use futures_lite::StreamExt;
            let data = self.get_mut();
            data.inner.poll_next(cx)
        }
    }

    /// A [`Sender`] [`Receiver`] paired together in a struct.
    #[derive(Clone)]
    pub struct Channel<T> {
        pub(crate) sender: async_broadcast::Sender<T>,
        pub(crate) receiver: async_broadcast::InactiveReceiver<T>,
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
        use crate::stream::StreamExt;

        #[test]
        fn can_sink_stream() {
            futures_lite::future::block_on(async {
                let (tx, mut rx) = bounded::<String>(1.try_into().unwrap());
                tx.send("hello".into()).await.unwrap();
                let _ = rx.next().await.unwrap();
            })
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use super::*;
    use crate::stream::StreamExt;

    #[test]
    fn channel_sinks_and_streams() {
        let (f32tx, f32rx) = broadcast::bounded::<f32>(3.try_into().unwrap());
        let f32stream = f32rx.map(|f| format!("{:.2}", f)).boxed();
        let (u32tx, u32rx) = broadcast::bounded::<u32>(3.try_into().unwrap());
        let u32stream = u32rx.map(|u| format!("{}", u)).boxed();
        let formatted = u32stream.or(f32stream);

        crate::future::block_on(async move {
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

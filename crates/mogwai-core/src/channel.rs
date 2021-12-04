//! Async mpmc and broadcast channels, plus extensions.

use std::{ops::DerefMut, sync::Arc};

pub use futures::channel::oneshot;
use futures::{future::Either, lock::Mutex, Future};

pub mod mpsc {
    //! Async multiple producer, single consumer channel.
    pub use futures::channel::mpsc::{*, channel as bounded};
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
    //! The channel can also be closed manually by calling [`Sender::close()`] or [`Receiver::close()`].
    pub use async_broadcast::{broadcast as bounded, *};

    /// Waits until the channel of the given `Sender` is empty.
    pub async fn until_empty<T>(tx: &Sender<T>) {
        while !tx.is_empty() {
            let _ = crate::time::wait_millis(1).await;
        }
    }

    /// A [`Sender`] [`Receiver`] paired together in a struct.
    #[derive(Clone)]
    pub struct Channel<T> {
        sender: Sender<T>,
        receiver: InactiveReceiver<T>,
    }

    impl<T> Channel<T> {
        /// Create a new broadcast channel with the given capacity.
        pub fn new(cap: usize) -> Self {
            let (sender, rx) = bounded(cap);
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
            self.sender.clone()
        }

        /// Create a new active Receiver out of this channel.
        pub fn receiver(&self) -> Receiver<T> {
            self.receiver.activate_cloned()
        }
    }
}

/// A captured future, which can be used to store the result of
pub struct Captured<T> {
    inner: Arc<Mutex<Either<Box<dyn Future<Output = T> + Unpin>, Option<T>>>>,
}

impl<T: Clone> Captured<T> {
    /// Create a new captured future.
    pub fn new(f: impl Future<Output = T> + Unpin + 'static) -> Self {
        Captured {
            inner: Arc::new(Mutex::new(Either::Left(Box::new(f)))),
        }
    }

    /// Await and return a clone of the inner `T`.
    pub async fn get(&self) -> T {
        loop {
            let mut lock = self.inner.lock().await;
            let either = std::mem::replace(lock.deref_mut(), Either::Right(None));
            let res = match either {
                Either::Left(rx) => Some(rx.await),
                Either::Right(r) => r,
            };

            *lock = Either::Right(res.clone());

            if let Some(t) = res {
                return t;
            }
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

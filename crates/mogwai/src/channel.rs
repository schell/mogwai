//! Async mpmc and broadcast channels, plus extensions.
pub mod mpmc {
    //! Async multi-producer multi-consumer channel, **where each message can be received by only
    //! one of all existing consumers**.
    //!
    //! For this reason it is recommended to use this flavor of channel
    //! for messages which are not `Clone` and you must be careful not to depend
    //! on more than **one** [`Receiver`] reading messages from the channel. A
    //! good use case for this channel is for sending [`ViewBuilder`][crate::builder::ViewBuilder]s to a parent
    //! view.
    //! For channels where each message is received by _all_ consumers see [`super::broadcast`].
    //!
    //! There are two kinds of MPMC channel:
    //!
    //! 1. [Bounded][`bounded()`] channel with limited capacity.
    //! 2. [Unbounded][`unbounded()`] channel with unlimited capacity.
    //!
    //! A channel is a [`Sender`] and [`Receiver`] pair. Both sides are cloneable and can be shared
    //! among multiple threads.
    //!
    //! When all [`Sender`]s or all [`Receiver`]s are dropped, the channel becomes closed. When a
    //! channel is closed, no more messages can be sent, but remaining messages can still be received.
    //!
    //! Additionally, [`Sender`] can be turned into a [`crate::futures::Sink`] and [`Receiver`] implements [`crate::futures::Stream`], both of
    //! which are used extensively by [`crate::builder::ViewBuilder`] to set up communication into and out of views.
    //!
    //! Please see the documentation for [`crate::futures::StreamExt`] and [`crate::futures::SinkExt`] to get acquanted with the various
    //! operations available when using channels.
    //!
    //! # Examples
    //!
    //! ```
    //! use mogwai::channel::*;
    //! futures::executor::block_on(async {
    //!     let (s, r) = mpmc::unbounded();
    //!
    //!     assert_eq!(s.send("Hello").await, Ok(()));
    //!     assert_eq!(r.recv().await, Ok("Hello"));
    //! });
    //! ```
    pub use async_channel::*;

    /// Waits until the channel of the given `Sender` is empty.
    pub async fn until_empty<T>(tx: &Sender<T>) {
        while !tx.is_empty() {
            let _ = crate::time::wait_approx(1.0).await;
        }
    }

    /// A [`Sender`] [`Receiver`] pair.
    pub type Channel<T> = (Sender<T>, Receiver<T>);
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
            let _ = crate::time::wait_approx(1.0).await;
        }
    }

    /// A [`Sender`] [`Receiver`] paired together in a struct.
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::futures::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn channel_sinks_and_streams() {
        let (f32tx, f32rx) = mpmc::bounded::<f32>(3);
        let f32stream = f32rx.map(|f| format!("{:.2}", f)).boxed();
        let (u32tx, u32rx) = broadcast::bounded::<u32>(3);
        let u32stream = u32rx.map(|u| format!("{}", u)).boxed();
        let formatted = futures::stream::select_all(vec![u32stream, f32stream]);

        f32tx.send(1.5).await.unwrap();
        u32tx.broadcast(666).await.unwrap();
        f32tx.send(2.3).await.unwrap();

        let mut strings: Vec<String> = formatted.take(3).collect::<Vec<_>>().await;
        strings.sort();

        assert_eq!(
            strings,
            vec!["1.50".to_string(), "2.30".to_string(), "666".to_string()]
        );
    }
}

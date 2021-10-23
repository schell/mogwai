//! Async mpmc and broadcast channels, plus extensions.
pub mod mpmc {
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
    pub use async_channel::*;

    /// Waits until the channel of the given `Sender` is empty.
    pub async fn until_empty<T>(tx: &Sender<T>) {
        while !tx.is_empty() {
            let _ = crate::time::wait_approx(0.01).await;
        }
    }
}

pub mod broadcast {
    //! Broadcast channels.
    pub use async_broadcast::{broadcast as bounded, *};

    /// Waits until the channel of the given `Sender` is empty.
    pub async fn until_empty<T>(tx: &Sender<T>) {
        while !tx.is_empty() {
            let _ = crate::time::wait_approx(0.01).await;
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

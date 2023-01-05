//! Types and extention traits for [`Sink`]s.
//!
//! Re-exports some of the futures crate, along with extensions and helper types.
use std::{future::Future, marker::PhantomData, pin::Pin, fmt::Display};

#[derive(Debug, Clone, Copy)]
pub enum TrySendError {
    // The sink is closed
    Closed,
    // The sink is full
    Full,
    // The sink is busy (eg. locked)
    Busy,
}

impl Display for TrySendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TrySendError::Closed => "sender is closed",
            TrySendError::Full => "sender is full",
            TrySendError::Busy => "sender is busy",
        })
    }
}

impl std::error::Error for TrySendError {}

#[derive(Debug, Clone, Copy)]
pub enum SendError {
    // The sink is closed
    Closed,
    // The sink is full
    Full,
}

impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SendError::Closed => "sender is closed",
            SendError::Full => "sender is full",
        })
    }
}

impl std::error::Error for SendError {}

pub trait Sink<Item> {
    fn send(
        &self,
        item: Item,
    ) -> Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + '_>>;

    fn try_send(&self, item: Item) -> Result<(), TrySendError>;
}

pub trait SinkExt<Item>: Sink<Item> {
    /// Extend this sink using a map function.
    ///
    /// This composes the map function _in front of the sink_, consuming a sink that takes
    /// `S` and returning a sink that takes `Item`.
    fn contra_map<S, F>(self, f: F) -> ContraMap<Self, S, Item, F>
    where
        Self: Sized,
        F: Fn(S) -> Item,
    {
        ContraMap {
            map: f,
            sink: self,
            _x: PhantomData,
            _y: PhantomData,
        }
    }

    /// Extend this sink using a filtering map function.
    ///
    /// This composes the filter map function _in front of the sink_, consuming a sink that takes
    /// `S` and returning a sink that takes `Item`.
    fn contra_filter_map<S, F>(self, f: F) -> ContraFilterMap<Self, S, Item, F>
    where
        Self: Sized,
        F: Fn(S) -> Option<Item>,
    {
        ContraFilterMap {
            map: f,
            sink: self,
            _x: PhantomData,
            _y: PhantomData,
        }
    }
}

impl<S: ?Sized + Sink<Item>, Item> Sink<Item> for Box<S> {
    fn send(
        &self,
        item: Item,
    ) -> Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + '_>> {
        (**self).send(item)
    }

    fn try_send(&self, item: Item) -> Result<(), TrySendError> {
        (**self).try_send(item)
    }
}

impl<Item, T: Sink<Item> + ?Sized> SinkExt<Item> for T {}

/// Type for supporting contravariant mapped sinks.
pub struct ContraMap<S, X, Y, F>
where
    F: Fn(X) -> Y,
{
    sink: S,
    map: F,
    _x: PhantomData<X>,
    _y: PhantomData<Y>,
}

impl<S, X, Y, F> Sink<X> for ContraMap<S, X, Y, F>
where
    S: Sink<Y> + Unpin,
    F: Fn(X) -> Y + Unpin,
    X: Unpin,
    Y: Unpin,
{
    fn send(
        &self,
        item: X,
    ) -> Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + '_>> {
        let item = (self.map)(item);
        self.sink.send(item)
    }

    fn try_send(&self, item: X) -> Result<(), TrySendError> {
        let item = (self.map)(item);
        self.sink.try_send(item)
    }
}

/// Type for supporting contravariant filter-mapped sinks.
pub struct ContraFilterMap<S, X, Y, F>
where
    F: Fn(X) -> Option<Y>,
{
    sink: S,
    map: F,
    _x: PhantomData<X>,
    _y: PhantomData<Y>,
}

impl<S, X, Y, F> Sink<X> for ContraFilterMap<S, X, Y, F>
where
    S: Sink<Y> + Unpin,
    F: Fn(X) -> Option<Y> + Unpin,
    X: Unpin,
    Y: Unpin,
{
    fn send(
        &self,
        item: X,
    ) -> Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + '_>> {
        if let Some(item) = (self.map)(item) {
            self.sink.send(item)
        } else {
            Box::pin(std::future::ready(Ok(())))
        }
    }

    fn try_send(&self, item: X) -> Result<(), TrySendError> {
        if let Some(item) = (self.map)(item) {
            self.sink.try_send(item)
        } else {
            Ok(())
        }
    }
}

/// Contravariant functor extensions for types that implement [`Sink`].
#[cfg(all(not(target_arch = "wasm32"), test))]
mod test {
    use crate::sink::{Sink, SinkExt};

    #[test]
    fn can_contra_map() {
        futures_lite::future::block_on(async {
            let (tx, mut rx) = crate::channel::broadcast::bounded::<String>(1.try_into().unwrap());

            // sanity
            tx.broadcast("blah".to_string()).await.unwrap();
            let _ = rx.recv().await.unwrap();

            let tx = tx.contra_map(|n: u32| format!("{}", n));
            tx.send(42).await.unwrap();
            let s = rx.recv().await.unwrap();
            assert_eq!(s.as_str(), "42");
        });
    }
}

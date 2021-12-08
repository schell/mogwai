//! Types and extention traits for [`Sink`]s.
//!
//! Re-exports some of the futures crate, along with extensions and helper types.
use std::marker::PhantomData;

pub use futures::sink::*;
use futures::task;

use crate::target::Sendable;

/// Type for supporting contravariant mapped sinks.
pub struct ContraMap<S, X, Y, F>
where
    F: Fn(X) -> Y + Sendable,
{
    sink: S,
    map: F,
    _x: PhantomData<X>,
    _y: PhantomData<Y>,
}

impl<S, X, Y, F> Sink<X> for ContraMap<S, X, Y, F>
where
    S: Sink<Y> + Unpin,
    F: Fn(X) -> Y + Sendable + Unpin,
    X: Unpin,
    Y: Unpin,
{
    type Error = <S as Sink<Y>>::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        futures::ready!(self.get_mut().sink.poll_ready_unpin(cx))?;
        std::task::Poll::Ready(Ok(()))
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: X) -> Result<(), Self::Error> {
        let data = self.get_mut();
        let item = (data.map)(item);
        data.sink.start_send_unpin(item)?;
        Ok(())
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        futures::ready!(self.get_mut().sink.poll_flush_unpin(cx))?;
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.get_mut().sink.poll_close_unpin(cx)
    }
}

/// Type for supporting contravariant filter-mapped sinks.
pub struct ContraFilterMap<S, X, Y, F>
where
    F: Fn(X) -> Option<Y> + Sendable,
{
    sink: S,
    map: F,
    _x: PhantomData<X>,
    _y: PhantomData<Y>,
}

impl<S, X, Y, F> Sink<X> for ContraFilterMap<S, X, Y, F>
where
    S: Sink<Y> + Unpin,
    F: Fn(X) -> Option<Y> + Sendable + Unpin,
    X: Unpin,
    Y: Unpin,
{
    type Error = <S as Sink<Y>>::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        futures::ready!(self.get_mut().sink.poll_ready_unpin(cx))?;
        std::task::Poll::Ready(Ok(()))
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: X) -> Result<(), Self::Error> {
        let data = self.get_mut();
        if let Some(item) = (data.map)(item) {
            data.sink.start_send_unpin(item)?;
        }
        Ok(())
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        futures::ready!(self.get_mut().sink.poll_flush_unpin(cx))?;
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.get_mut().sink.poll_close_unpin(cx)
    }
}

/// Contravariant functor extensions for types that implement [`Sink`].
pub trait Contravariant<T>: Sink<T> + Sized {
    /// Extend this sink using a map function.
    ///
    /// This composes the map function _in front of the sink_, much like [`SinkExt::with`]
    /// but without async and without the option of failure.
    fn contra_map<S, F>(self, f: F) -> ContraMap<Self, S, T, F>
    where
        F: Fn(S) -> T + Sendable
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
    /// This composes the map function _in front of the sink_, much like [`SinkExt::with_flat_map`]
    /// but without async and without the option of failure.
    fn contra_filter_map<S, F>(
        self,
        f: F,
    ) -> ContraFilterMap<Self, S, T, F>
    where
        F: Fn(S) -> Option<T> + Sendable
    {
        ContraFilterMap {
            map: f,
            sink: self,
            _x: PhantomData,
            _y: PhantomData
        }
    }
}

impl<S: Sized, T> Contravariant<T> for S where S: Sink<T> {}

#[cfg(all(not(target_arch = "wasm32"), test))]
mod test {
    use crate::futures::{SinkExt, sink::Contravariant};

    #[test]
    fn can_contra_map() {
        smol::block_on(async {
            let (tx, mut rx) = crate::channel::broadcast::bounded::<String>(1);

            // sanity
            tx.broadcast("blah".to_string()).await.unwrap();
            let _ = rx.recv().await.unwrap();

            let mut tx = tx.clone().contra_map(|n: u32| format!("{}", n));
            tx.send(42).await.unwrap();
            let s = rx.recv().await.unwrap();
            assert_eq!(s.as_str(), "42");
        });
    }

}

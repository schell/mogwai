//! Futures, streams, sinks.
//!
//! Re-exports of the futures crate, along with extensions and helper types.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use futures::task;
pub use futures::{sink, stream, Sink, SinkExt, Stream, StreamExt};

use crate::target::Sendable;

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

/// Type for supporting contravariant mapped sinks.
pub struct ContraMap<S, X, Y> {
    sink: S,
    #[cfg(target_arch = "wasm32")]
    map: Box<dyn Fn(X) -> Y + 'static>,

    #[cfg(not(target_arch = "wasm32"))]
    map: Box<dyn Fn(X) -> Y + Send + 'static>,
}

impl<S: Sink<Y> + Unpin, X, Y> Sink<X> for ContraMap<S, X, Y> {
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
pub struct ContraFilterMap<S, X, Y> {
    sink: S,
    #[cfg(target_arch = "wasm32")]
    map: Box<dyn Fn(X) -> Option<Y> + 'static>,

    #[cfg(not(target_arch = "wasm32"))]
    map: Box<dyn Fn(X) -> Option<Y> + Send + 'static>,
}

impl<S: Sink<Y> + Unpin, X, Y> Sink<X> for ContraFilterMap<S, X, Y> {
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
    fn contra_map<S>(self, f: impl Fn(S) -> T + Sendable) -> ContraMap<Self, S, T> {
        ContraMap {
            map: Box::new(f),
            sink: self,
        }
    }

    /// Extend this sink using a filtering map function.
    ///
    /// This composes the map function _in front of the sink_, much like [`SinkExt::with_flat_map`]
    /// but without async and without the option of failure.
    fn contra_filter_map<S>(
        self,
        f: impl Fn(S) -> Option<T> + Sendable,
    ) -> ContraFilterMap<Self, S, T> {
        ContraFilterMap {
            map: Box::new(f),
            sink: self,
        }
    }
}

impl<S: Sized, T> Contravariant<T> for S where S: Sink<T> {}

#[cfg(all(not(target_arch = "wasm32"), test))]
mod test {
    use super::{ContraMap, Contravariant, IntoSenderSink, SinkExt};

    #[test]
    fn can_contra_map() {
        smol::block_on(async {
            let (tx, mut rx) = crate::channel::broadcast::bounded::<String>(1);

            // sanity
            tx.broadcast("blah".to_string()).await.unwrap();
            let _ = rx.recv().await.unwrap();

            let mut tx: ContraMap<_, u32, String> = tx.sink().contra_map(|n: u32| format!("{}", n));
            tx.send(42).await.unwrap();
            let s = rx.recv().await.unwrap();
            assert_eq!(s.as_str(), "42");
        });
    }
}

pub use futures::{Sink, SinkExt, Stream, StreamExt};
use std::collections::VecDeque;

use crate::var::{self, Counted, Shared};

/// The sending side of a channel.
///
/// A simple wrapper around [`async_channel::Sender`] with some extra fields
/// to help implement `Sink`.
pub struct Sender<T> {
    sender: async_channel::Sender<T>,
    sending_msgs: Counted<Shared<VecDeque<T>>>,
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
    // Receiver is closed.
    Closed,
    // The channel is full
    Full,
}

impl<T: 'static> Sender<T> {
    fn flush_sink(&mut self) -> Result<(), SinkError> {
        if self.sender.is_closed() {
            return Err(SinkError::Closed);
        }

        while let Some(item) = self.sending_msgs.visit_mut(|ts| ts.pop_front()) {
            match self.sender.try_send(item) {
                Ok(()) => {}
                Err(err) => match err {
                    async_channel::TrySendError::Full(t) => {
                        self.sending_msgs.visit_mut(move |ts| ts.push_front(t));
                        return Err(SinkError::Full);
                    }
                    async_channel::TrySendError::Closed(t) => {
                        self.sending_msgs.visit_mut(move |ts| ts.push_front(t));
                        return Err(SinkError::Closed);
                    }
                },
            }
        }

        assert!(self.sending_msgs.visit(VecDeque::is_empty));
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

        if cap.is_none() || cap.unwrap() > self.sending_msgs.visit(|ts| ts.len()) {
            std::task::Poll::Ready(Ok(()))
        } else {
            // There are already messages in the queue
            std::task::Poll::Pending
        }
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        let data = self.get_mut();

        if data.sender.is_closed() {
            return Err(SinkError::Closed);
        }

        let item = data.sending_msgs.visit_mut(|ts| {
            ts.push_back(item);
            ts.pop_front().unwrap()
        });

        match data.sender.try_send(item) {
            Ok(()) => Ok(()),
            Err(async_channel::TrySendError::Full(t)) => {
                data.sending_msgs.visit_mut(|ts| ts.push_front(t));
                Ok(())
            }
            Err(async_channel::TrySendError::Closed(t)) => {
                data.sending_msgs.visit_mut(|ts| ts.push_front(t));
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

pub fn bounded<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = async_channel::bounded::<T>(cap);
    (
        Sender {
            sender: tx,
            sending_msgs: var::new(VecDeque::default()),
        },
        Receiver(rx),
    )
}

pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = async_channel::unbounded::<T>();
    (
        Sender {
            sender: tx,
            sending_msgs: var::new(VecDeque::default()),
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

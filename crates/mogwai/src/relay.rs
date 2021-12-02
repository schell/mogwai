//! Easy communication with views.
//!
//! Views are sometimes very complex. Many times we either have to
//! manage a large number of channels or a small number of channels
//! that send messages with many enum variants. Both cases can become
//! overwhelming.
//!
//! To help with this situation `mogwai >= 0.5.2` has introduced the
//! concept of view "relays". A view relay is an object that uses
//! inputs and outputs to manage communicating updates and events
//! to and from a view. Instead of having to know the intricacies
//! of a number of different channels and their operating behavior,
//! the library user creates a struct that defines inputs and outputs
//! and uses those to construct a [`ViewBuilder`], then interacts with
//! that struct from within their logic loop to communicate with the
//! view.
use futures::{Sink, SinkExt, Stream, StreamExt};

use crate::{
    channel::broadcast,
    event::Eventable,
    futures::{IntoSenderSink, SinkError},
    target::Sendable,
};

/// An input to a view.
///
/// An input has at most one consumer in the destination view.
pub struct Input<T> {
    setter: futures::channel::mpsc::Sender<T>,
    rx: Option<futures::channel::mpsc::Receiver<T>>,
}

impl<T> Default for Input<T> {
    fn default() -> Self {
        let (setter, getter) = futures::channel::mpsc::channel(1);
        Self {
            setter,
            rx: Some(getter),
        }
    }
}

impl<T: Sendable> Clone for Input<T> {
    fn clone(&self) -> Self {
        Self {
            setter: self.setter.clone(),
            rx: None,
        }
    }
}

impl<T: Sendable> Input<T> {
    /// Set the value of this input.
    pub async fn set(&self, item: impl Into<T>) -> Result<(), ()> {
        let mut setter = self.setter.clone();
        setter.send(item.into()).await.map_err(|_| ())
    }

    /// Attempt to acquire a stream of updates to this input.
    ///
    /// An `Input` can have at most **one** consumer in the destination view.
    /// For this reason this function returns `Some` the first time it is called,
    /// and `None` each subsequent call.
    ///
    /// It is suggested you use `input.stream().unwrap()` (or similar) when constructing
    /// a [`ViewBuilder`] from an `Input` so that the program fails if this function is
    /// called more than once on the same input.
    pub fn stream(&mut self) -> Option<impl Stream<Item = T>> {
        let rx = self.rx.take();
        rx
    }
}

/// An event output from a view.
#[derive(Clone)]
pub struct Output<T> {
    chan: broadcast::Channel<T>,
}

impl<T> Default for Output<T> {
    fn default() -> Self {
        let mut chan = broadcast::Channel::new(1);
        chan.set_overflow(true);
        Self { chan }
    }
}

impl<T: Sendable + Clone + Unpin> Output<T> {
    /// Attempt to send an event through the output syncronously.
    ///
    /// This can be used by views to send events downstream.
    pub fn try_send(&self, item: impl Into<T>) -> Result<(), ()> {
        let item = item.into();
        let tx = self.chan.sender();
        tx.try_broadcast(item).map(|_| ()).map_err(|_| ())
    }

    /// Returns a sink used to send events through the output.
    ///
    /// This can be used by views to send events downstream.
    pub fn sink(&self) -> impl Sink<T, Error = SinkError> {
        self.chan.sender().sink()
    }

    /// Return the next event occurrence.
    ///
    /// A returned value of `None` means the output is no longer
    /// operating.
    pub async fn get(&self) -> Option<T> {
        let mut rx = self.chan.receiver();
        rx.next().await
    }

    /// Return a stream of event occurrences.
    pub fn get_stream(&self) -> impl Stream<Item = T> {
        self.chan.receiver()
    }

    /// Convert the output into stream of event occurrences.
    pub fn into_stream(self) -> impl Stream<Item = T> {
        self.chan.receiver()
    }
}

/// Marker trait to aid in writing relays.
pub trait RelayView: Eventable + Sendable + Clone + Unpin {}
impl<T> RelayView for T where T: Eventable + Sendable + Clone + Unpin {}

/// Marker trait to aid in writing relays.
pub trait RelayEvent: Sendable + Clone + Unpin {}
impl<T> RelayEvent for T where T: Sendable + Clone + Unpin {}
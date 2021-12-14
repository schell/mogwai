//! Bundling view updates and events.
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
//! and uses those to construct a [`ViewBuilder`](crate::builder::ViewBuilder).
//! Updates to the view are then made by interacting with the relay struct
//! asyncronously from within a logic loop.
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
};

use futures::{Sink, SinkExt, Stream, StreamExt};

use crate::{
    builder::ViewBuilder,
    channel::{broadcast, SinkError},
    component::Component,
    event::Eventable,
    target::{Sendable, Spawnable},
    view::View,
};

/// An input to a view.
///
/// An input has at most one consumer in the destination view.
pub struct Input<T> {
    setter: futures::channel::mpsc::Sender<T>,
    rx: Arc<Mutex<Option<futures::channel::mpsc::Receiver<T>>>>,
}

impl<T> Default for Input<T> {
    fn default() -> Self {
        let (setter, getter) = futures::channel::mpsc::channel(1);
        Self {
            setter,
            rx: Arc::new(Mutex::new(Some(getter))),
        }
    }
}

impl<T: Sendable> Clone for Input<T> {
    fn clone(&self) -> Self {
        Self {
            setter: self.setter.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl<T: Sendable> Input<T> {
    /// Set the value of this input.
    pub async fn set(&self, item: impl Into<T>) -> Result<(), ()> {
        let mut setter = self.setter.clone();
        setter.send(item.into()).await.map_err(|_| ())
    }

    /// Attempt to set the value of this input syncronously.
    ///
    /// When this fails it is because the input has an existing value
    /// set that has not been consumed.
    pub fn try_set(&mut self, item: impl Into<T>) -> Result<(), ()> {
        self.setter.try_send(item.into()).map_err(|_| ())
    }

    /// Attempt to acquire a stream of updates to this input.
    ///
    /// An `Input` can have at most **one** consumer in the destination view.
    /// For this reason this function returns `Some` the first time it is called,
    /// and `None` each subsequent call.
    ///
    /// It is suggested you use `input.stream().unwrap()` (or similar) when constructing
    /// a [`ViewBuilder`](crate::builder::ViewBuilder) from an `Input` so that
    /// the program fails if this function is called more than once on the same input.
    pub fn stream(&mut self) -> Option<impl Stream<Item = T>> {
        let mut lock = self.rx.lock().unwrap();
        lock.take()
    }
}

/// An input that fans input values to many consumers.
///
/// A fan input may have many consumers in the destination view,
/// for this reason values must be `Clone`.
pub struct FanInput<T> {
    chan: broadcast::Channel<T>,
}

impl<T> Default for FanInput<T> {
    fn default() -> Self {
        Self {
            chan: broadcast::Channel::new(1),
        }
    }
}

impl<T: Clone> Clone for FanInput<T> {
    fn clone(&self) -> Self {
        Self {
            chan: self.chan.clone(),
        }
    }
}

impl<T: Clone> FanInput<T> {
    /// Set the value of this input.
    pub async fn set(&self, item: impl Into<T>) -> Result<(), ()> {
        let mut setter = self.chan.sender();
        setter.send(item.into()).await.map_err(|_| ())
    }

    /// Attempt to set the value of this input syncronously.
    ///
    /// When this fails it is because the input has no destination and
    /// the underlying channel is closed.
    pub fn try_set(&mut self, item: impl Into<T>) -> Result<(), ()> {
        let setter = self.chan.sender();
        setter
            .inner
            .try_broadcast(item.into())
            .map_err(|_| ())
            .map(|_| ())
    }

    /// Attempt to acquire a stream of updates to this input.
    ///
    /// Unlike `Input`, `FanInput` can have many consumers in the destination view,
    /// so this operation always returns a stream.
    pub fn stream(&self) -> impl Stream<Item = T> {
        self.chan.receiver()
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
        tx.inner.try_broadcast(item).map(|_| ()).map_err(|_| ())
    }

    /// Returns a sink used to send events through the output.
    ///
    /// This can be used by views to send events downstream.
    pub fn sink(&self) -> impl Sink<T, Error = SinkError> {
        self.chan.sender()
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

/// Helper trait that allows a relay to be directly converted into a [`Component`].
pub trait Relay<T>
where
    T: RelayView,
    T::Event: RelayEvent,
    Self: Sendable + Sized,
    Self::Error: Sendable + std::fmt::Debug,
    View<T>: TryFrom<ViewBuilder<T>>,
{
    type Error;

    /// Create a view builder.
    ///
    /// `self` is borrowed as mutable to allow using [`Input::stream`] on all inputs.
    fn view(&mut self) -> ViewBuilder<T>;

    /// Convert the relay into an asynchronous logic loop.
    fn logic(self) -> Pin<Box<dyn Spawnable<Result<(), Self::Error>>>> {
        Box::pin(futures::future::ready(Ok(())))
    }

    /// Convert into a component.
    fn into_component(mut self) -> Component<T> {
        let builder = self.view();
        let logic = self.logic();
        Component::from(builder).with_logic(async move {
            if let Err(err) = logic.await {
                log::error!("relay run error: {:?}", err);
            }
        })
    }

    /// Convert directly into a view.
    fn into_view(self) -> Result<View<T>, <View<T> as TryFrom<ViewBuilder<T>>>::Error> {
        let comp = self.into_component();
        comp.build()
    }
}

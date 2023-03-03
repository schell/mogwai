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
//! and uses those to construct a [`ViewBuilder`](crate::view::ViewBuilder).
//! Updates to the view are then made by interacting with the relay struct
//! asyncronously from within a logic loop.
//!
//! ## Example
//! ```rust
//! use mogwai::prelude::*;
//!
//! #[derive(Default)]
//! struct ClickyDiv {
//!     click: Output<()>,
//!     text: Input<String>,
//! }
//!
//! impl TryFrom<ClickyDiv> for ViewBuilder {
//!     type Error = anyhow::Error;
//!
//!     fn try_from(mut cd: ClickyDiv) -> anyhow::Result<ViewBuilder> {
//!         Ok(ViewBuilder::element("div")
//!             .with_event("click", "myself", cd.click.sink().contra_map(|_: AnyEvent| ()))
//!             .append(
//!                 ("Hi", cd.text.stream().ok_or_else(|| anyhow::anyhow!("already used text stream"))?)
//!             )
//!             .with_task(async move {
//!                 let mut clicks = 0;
//!                 while let Some(()) = cd.click.get().await {
//!                     clicks += 1;
//!                     cd.text
//!                         .set(if clicks == 1 {
//!                             "1 click.".to_string()
//!                         } else {
//!                             format!("{} clicks.", clicks)
//!                         })
//!                         .await
//!                         .unwrap()
//!                 }
//!             })
//!         )
//!     }
//! }
//!
//! let cd = ClickyDiv::default();
//! let builder = ViewBuilder::try_from(cd).unwrap();
//! ```
use std::sync::{Arc, Mutex};

use anyhow::Context;

use crate::{
    channel::broadcast,
    sink::{SendError, Sink, TrySendError},
    stream::{Stream, StreamExt},
};

/// An input to a view.
///
/// `Input` has at most one consumer in the destination view.
pub struct Input<T> {
    setter: crate::channel::mpsc::Sender<T>,
    rx: Arc<Mutex<Option<crate::channel::mpsc::Receiver<T>>>>,
}

impl<T> Default for Input<T> {
    fn default() -> Self {
        let (setter, getter) = crate::channel::mpsc::bounded(1);
        Self {
            setter,
            rx: Arc::new(Mutex::new(Some(getter))),
        }
    }
}

impl<T> Clone for Input<T> {
    fn clone(&self) -> Self {
        Self {
            setter: self.setter.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl<T: Send> Sink<T> for Input<T> {
    fn send(
        &self,
        item: T,
    ) -> std::pin::Pin<Box<dyn futures_lite::Future<Output = Result<(), SendError>> + Send + '_>>
    {
        let item = item.into();
        Box::pin(async move { self.setter.send(item).await.map_err(|_| SendError::Closed) })
    }

    fn try_send(&self, item: T) -> Result<(), crate::prelude::TrySendError> {
        self.setter.try_send(item.into()).map_err(|e| match e {
            async_channel::TrySendError::Closed(_) => TrySendError::Closed,
            async_channel::TrySendError::Full(_) => TrySendError::Full,
        })
    }
}

impl<T: Send> Input<T> {
    /// Create a new input with a value already set.
    pub fn new(item: T) -> Self {
        let input = Self::default();
        // UNWRAP: safe because we know the channel has one empty slot
        input.setter.try_send(item).unwrap();
        input
    }

    /// Set the value of this input.
    pub async fn set(&self, item: impl Into<T>) -> anyhow::Result<()> {
        self.send(item.into()).await.map_err(|e| {
            anyhow::anyhow!(
                "could not set input of {}: {}",
                std::any::type_name::<T>(),
                e
            )
        })
    }

    /// Attempt to set the value of this input syncronously.
    ///
    /// When this fails it is because the input has an existing value
    /// set that has not been consumed.
    pub fn try_set(&self, item: impl Into<T>) -> anyhow::Result<()> {
        self.try_send(item.into())
            .ok()
            .with_context(|| format!("could not try_set input of {}", std::any::type_name::<T>()))
    }

    /// Attempt to acquire a stream of updates to this input.
    ///
    /// An `Input` can have at most **one** consumer.
    /// For this reason this function returns `Some` the first time it is called,
    /// and `None` each subsequent call.
    ///
    /// If you need more than one consumer for this stream, use [`FanInput`] instead.
    ///
    /// It is suggested you use `input.stream().unwrap()` (or similar) when constructing
    /// a [`ViewBuilder`](crate::view::ViewBuilder) from an `Input` so that
    /// the program fails if this function is called more than once on the same input.
    pub fn stream(&mut self) -> Option<impl Stream<Item = T> + Send> {
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

impl<T: Clone + Send + Sync> Sink<T> for FanInput<T> {
    fn send(
        &self,
        item: T,
    ) -> std::pin::Pin<Box<dyn futures_lite::Future<Output = Result<(), SendError>> + Send + '_>>
    {
        let item = item.into();
        let sender = self.chan.sender();
        Box::pin(async move { sender.send(item).await })
    }

    fn try_send(&self, item: T) -> Result<(), crate::prelude::TrySendError> {
        self.chan.sender().try_send(item.into())
    }
}

impl<T: Clone + Send + Sync> FanInput<T> {
    /// Set the value of this input.
    pub async fn set(&self, item: impl Into<T>) -> anyhow::Result<()> {
        self.send(item.into()).await.map_err(|e| {
            anyhow::anyhow!(
                "could not set fan input of {}: {}",
                std::any::type_name::<T>(),
                e
            )
        })
    }

    /// Attempt to set the value of this input syncronously.
    ///
    /// When this fails it is because the input has no destination and
    /// the underlying channel is closed.
    pub fn try_set(&mut self, item: impl Into<T>) -> anyhow::Result<()> {
        self.try_send(item.into())
            .ok()
            .with_context(|| format!("could not try_set input of {}", std::any::type_name::<T>()))
    }

    /// Attempt to acquire a stream of updates to this input.
    ///
    /// Unlike `Input`, `FanInput` can have many consumers in the destination view,
    /// so this operation always returns a stream.
    pub fn stream(&self) -> impl Stream<Item = T> + Send + Sync {
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

impl<T: Clone + Send + Sync> Sink<T> for Output<T> {
    fn send(
        &self,
        item: T,
    ) -> std::pin::Pin<Box<dyn futures_lite::Future<Output = Result<(), SendError>> + Send + '_>>
    {
        let sender = self.chan.sender();
        Box::pin(async move { sender.send(item).await })
    }

    fn try_send(&self, item: T) -> Result<(), TrySendError> {
        let sender = self.chan.sender();
        sender.try_send(item)
    }
}

impl<T: Clone + Send + Sync> Output<T> {
    /// Returns a sink used to send events through the output.
    ///
    /// This can be used by views to send events downstream.
    pub fn sink(&self) -> impl Sink<T> + Send + Sync {
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
    pub fn get_stream(&self) -> impl Stream<Item = T> + Send + Sync {
        self.chan.receiver()
    }

    /// Convert the output into stream of event occurrences.
    pub fn into_stream(self) -> impl Stream<Item = T> + Send + Sync {
        self.chan.receiver()
    }
}

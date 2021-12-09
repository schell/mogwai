//! Support for async operations depending on the target.
use futures::{Sink, Stream};

/// Marker trait for sending async messages.
#[cfg(target_arch = "wasm32")]
mod send {
    use futures::{Future, Sink, Stream};

    /// Marker trait for sending async messages.
    pub trait Sendable: 'static {}

    impl<T: 'static> Sendable for T {}

    /// Marker trait for streaming async messages.
    pub trait Syncable: 'static {}

    impl<T: 'static> Syncable for T {}

    /// Stream type alias.
    pub type Streaming<T> = dyn Stream<Item = T> + 'static;

    /// Sink type alias.
    pub type SinkingWith<T, E> = dyn Sink<T, Error = E> + 'static;

    /// FnMut to perform after a view type has been contstructed
    pub type PostBuild<T> = dyn FnOnce(&mut T) + 'static;

    /// Marker trait for futures that can be spawned.
    pub trait Spawnable<T>: Future<Output = T> + 'static {}
    impl<S, T: Future<Output = S> + 'static> Spawnable<S> for T {}
}

/// Marker trait for sending async messages.
#[cfg(not(target_arch = "wasm32"))]
mod send {
    use futures::{Future, Sink, Stream};

    /// Marker trait for streaming async messages.
    pub trait Sendable: Sized + Send + 'static {}

    impl<T: Send + 'static> Sendable for T {}

    /// Marker trait for streaming async messages.
    pub trait Syncable: Sized + Sync + 'static {}

    impl<T: Sync + 'static> Syncable for T {}

    /// Stream type alias.
    pub type Streaming<T> = dyn Stream<Item = T> + Send + 'static;

    /// Sink type alias.
    pub type SinkingWith<T, E> = dyn Sink<T, Error = E> + Send + 'static;

    /// FnMut to perform after a view type has been contstructed
    pub type PostBuild<T> = dyn FnOnce(&mut T) + Send + 'static;

    /// Marker trait for futures that can be spawned.
    pub trait Spawnable<T>: Future<Output = T> + Send + 'static {}
    impl<S, T: Future<Output = S> + Send + 'static> Spawnable<S> for T {}
}

pub use send::*;

use crate::channel::SinkError;

/// Sink type alias.
pub type Sinking<T> = SinkingWith<T, SinkError>;

/// Marker trait for streaming async messages.
pub trait Streamable<T>: Stream<Item = T> + Sendable {}
impl<T, C: Stream<Item = T> + Sendable> Streamable<T> for C {}

/// Marker trait for sinking/sending async messages.
pub trait Sinkable<T>: Sink<T, Error = SinkError> + Sendable {}
impl<T, C: Sink<T, Error = SinkError> + Sendable> Sinkable<T> for C {}

/// Spawn an async operation.
// TODO: Make spawn settable.
pub fn spawn<Fut>(fut: Fut)
where
    Fut: Spawnable<()>,
{
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(fut)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let task = smol::spawn(fut);
        task.detach();
    }
}

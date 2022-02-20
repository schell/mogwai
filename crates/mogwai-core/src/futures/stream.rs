//! Types and extention traits for [`Stream`]s.
//!
//! Re-exports some of the futures crate, along with extensions and helper types.
// TODO: kill this module
use std::pin::Pin;

pub use futures::stream::*;

impl<T: ?Sized> StreamableExt for T where T: Stream {}

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedStreamLocal<'a, T> = Pin<Box<dyn Stream<Item = T> + Send + Sync + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type BoxedStreamLocal<'a, T> = Pin<Box<dyn Stream<Item = T> + 'a>>;

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedStream<T> = Pin<Box<dyn Stream<Item = T> + Send + Sync + 'static>>;
#[cfg(target_arch = "wasm32")]
pub type BoxedStream<T> = Pin<Box<dyn Stream<Item = T> + 'static>>;

#[cfg(not(target_arch = "wasm32"))]
pub trait StreamableExt {
    fn pinned_local<'a>(self) -> BoxedStreamLocal<'a, Self::Item>
    where
        Self: Sized + Send + Sync + Stream + 'a
    {
        Box::pin(self)
    }

    fn pinned(self) -> BoxedStream<Self::Item>
    where
        Self: Sized + Send + Sync + Stream + 'static
    {
        Box::pin(self)
    }

}

#[cfg(target_arch = "wasm32")]
pub trait StreamableExt {
    fn pinned_local<'a>(self) -> BoxedStreamLocal<'a, Self::Item>
    where
        Self: Sized + Stream + 'a
    {
        Box::pin(self)
    }

    fn pinned(self) -> BoxedStream<Self::Item>
    where
        Self: Sized + Stream + 'static
    {
        Box::pin(self)
    }

}

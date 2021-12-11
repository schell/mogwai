//! Types and extention traits for [`Stream`]s.
//!
//! Re-exports some of the futures crate, along with extensions and helper types.
use std::pin::Pin;

pub use futures::stream::*;

impl<T: ?Sized> StreamableExt for T where T: Stream {}

#[cfg(not(target_arch = "wasm32"))]
pub trait StreamableExt {
    fn pinned_local<'a>(self) -> Pin<Box<dyn Stream<Item = Self::Item> + Send + 'a>>
    where
        Self: Sized + Send + Stream + 'a
    {
        Box::pin(self)
    }

    fn pinned(self) -> Pin<Box<dyn Stream<Item = Self::Item> + Send + 'static>>
    where
        Self: Sized + Send + Stream + 'static
    {
        Box::pin(self)
    }

}

#[cfg(target_arch = "wasm32")]
pub trait StreamableExt {
    fn pinned_local<'a>(self) -> Pin<Box<dyn Stream<Item = Self::Item> + 'a>>
    where
        Self: Sized + Stream + 'a
    {
        Box::pin(self)
    }

    fn pinned(self) -> Pin<Box<dyn Stream<Item = Self::Item> + 'static>>
    where
        Self: Sized + Stream + 'static
    {
        Box::pin(self)
    }

}

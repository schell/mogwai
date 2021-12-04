//! Wrapped views.
use std::ops::{Deref, DerefMut};

pub use futures::future::Either;

/// A wrapper around a domain-specific view.
pub struct View<T> {
    /// The underlying domain-specific view type.
    pub inner: T,
}

impl<T: Clone> View<T> {
    /// Convert the view into its inner type without detaching the view.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Deref for View<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for View<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

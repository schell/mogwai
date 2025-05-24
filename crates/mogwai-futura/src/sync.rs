//! Synchronization primitives.

use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

/// A "shared" value.
///
/// Equivalent to `Arc<RwLock<T>>`.
#[derive(Default)]
pub struct Shared<T> {
    inner: std::sync::Arc<std::sync::RwLock<T>>,
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: 'static> From<T> for Shared<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Shared<T> {
    /// Create a new shared `T`.
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(value)),
        }
    }

    /// Get a reference to the inner `T`.
    pub fn get(&self) -> impl Deref<Target = T> {
        self.inner.read().unwrap()
    }

    /// Get a mutable reference to the inner `T`.
    pub fn get_mut(&self) -> impl DerefMut<Target = T> {
        self.inner.write().unwrap()
    }

    /// Set the inner `T`.
    ///
    /// Returns the previous value.
    pub fn set(&self, value: T) -> T {
        let mut guard = self.inner.write().unwrap();
        std::mem::replace(guard.deref_mut(), value)
    }
}

//! Synchronization primitives.

use std::ops::{Deref, DerefMut};

/// A "shared" value.
///
/// Equivalent to `Arc<RwLock<T>>`.
#[derive(Default)]
pub struct Shared<T> {
    #[cfg(not(target_arch = "wasm32"))]
    inner: std::sync::Arc<std::sync::RwLock<T>>,
    #[cfg(target_arch = "wasm32")]
    inner: std::rc::Rc<std::cell::RefCell<T>>,
}

impl<T: PartialEq> PartialEq for Shared<T> {
    #[cfg(not(target_arch = "wasm32"))]
    fn eq(&self, other: &Self) -> bool {
        self.inner.read().unwrap().eq(&other.inner.read().unwrap())
    }
    #[cfg(target_arch = "wasm32")]
    fn eq(&self, other: &Self) -> bool {
        self.inner.borrow().eq(&other.inner.borrow())
    }
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
            #[cfg(not(target_arch = "wasm32"))]
            inner: std::sync::Arc::new(std::sync::RwLock::new(value)),
            #[cfg(target_arch = "wasm32")]
            inner: std::rc::Rc::new(std::cell::RefCell::new(value)),
        }
    }

    /// Get a reference to the inner `T`.
    pub fn get(&self) -> impl Deref<Target = T> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.read().unwrap()
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.inner.borrow()
        }
    }

    /// Get a mutable reference to the inner `T`.
    pub fn get_mut(&self) -> impl DerefMut<Target = T> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.write().unwrap()
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.inner.borrow_mut()
        }
    }

    /// Set the inner `T`.
    ///
    /// Returns the previous value.
    pub fn set(&self, value: T) -> T {
        std::mem::replace(self.get_mut().deref_mut(), value)
    }
}

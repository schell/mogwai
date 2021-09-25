//! Variables
use std::ops::Deref;

/// An abstraction over [`std::sync::Arc`] or [`std::rc::Rc`], depending on configuration and targets.
#[derive(Debug, Default)]
pub struct Counted<T> {
    #[cfg(target_arch = "wasm32")]
    inner: std::rc::Rc<T>,
    #[cfg(not(target_arch = "wasm32"))]
    inner: std::sync::Arc<T>,
}

impl<T> Clone for Counted<T> {
    fn clone(&self) -> Self {
        Counted {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Deref for Counted<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> Counted<T> {
    #[cfg(target_arch = "wasm32")]
    pub fn new(t: T) -> Self {
        Counted {
            inner: std::rc::Rc::new(t),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(t: T) -> Self {
        Counted {
            inner: std::sync::Arc::new(t),
        }
    }
}

/// An abstraction over [`std::sync::Mutex`] or [`std::cell::RefCell`], depending on configuration and targets.
#[derive(Default)]
pub struct Shared<T> {
    #[cfg(target_arch = "wasm32")]
    inner: std::cell::RefCell<T>,
    #[cfg(not(target_arch = "wasm32"))]
    inner: std::sync::Mutex<T>,
}

impl<T> Shared<T> {
    /// Create a new shared variable.
    #[cfg(target_arch = "wasm32")]
    pub fn new(t: T) -> Self {
        Shared {
            inner: std::cell::RefCell::new(t),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(t: T) -> Self {
        Shared {
            inner: std::sync::Mutex::new(t),
        }
    }

    /// Visit the value of the shared variable using a closure
    /// which may return a value.
    #[cfg(target_arch = "wasm32")]
    pub fn visit<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&T) -> A,
    {
        f(&self.inner.borrow())
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn visit<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&T) -> A,
    {
        f(&self.inner.lock().unwrap())
    }

    /// Visit the value of the shared variable using a closure
    /// which may mutate the inner value and return a value.
    #[cfg(target_arch = "wasm32")]
    pub fn visit_mut<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&mut T) -> A,
    {
        f(&mut self.inner.borrow_mut())
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn visit_mut<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&mut T) -> A,
    {
        f(&mut self.inner.lock().unwrap())
    }
}

pub fn new<T>(t:T) -> Counted<Shared<T>> {
    Counted::new(Shared::new(t))
}

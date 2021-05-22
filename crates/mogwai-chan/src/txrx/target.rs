//! Conditionally compiled code.
use std::{collections::HashMap, future::Future, pin::Pin, sync::atomic::AtomicUsize};

#[cfg(not(target_arch = " wasm32"))]
pub trait Transmission: Send + Sync + 'static {}
#[cfg(target_arch = "wasm32")]
pub trait Transmission: 'static {}

#[cfg(not(target_arch = " wasm32"))]
impl<T: Send + Sync + 'static> Transmission for T {}
#[cfg(target_arch = "wasm32")]
impl<T: 'static> Transmission for T {}

pub trait FutureMessage<A>: Future<Output = A> + Send + 'static {}
impl<T: Future<Output = A> + Send + 'static, A> FutureMessage<A> for T {}

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

impl<T> Counted<T> {
    #[cfg(target_arch = "wasm32")]
    fn new(t: T) -> Self {
        Counted {
            inner: std::Rc::new(t),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn new(t: T) -> Self {
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

#[cfg(target_arch = "wasm32")]
type Response<A> = Box<dyn FnMut(&A)>;
#[cfg(not(target_arch = "wasm32"))]
type Response<A> = Box<dyn FnMut(&A) + Send + Sync>;

pub struct Responders<A> {
    next_k: AtomicUsize,
    branches: Shared<HashMap<usize, Response<A>>>,
}

impl<A> Default for Responders<A> {
    fn default() -> Self {
        Self {
            next_k: AtomicUsize::new(0),
            branches: Default::default(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl<A> Responders<A> {
    pub fn insert(&self, k: usize, f: impl FnMut(&A) + Transmission) {
        todo!()
    }

    pub fn remove(&self, k: usize) {
        todo!()
    }

    pub fn send(&self, a: &A) {
        todo!()
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl<A> Responders<A> {
    pub fn insert(&self, k: usize, f: impl FnMut(&A) + Transmission) {
        let mut guard = self.branches.inner.lock().unwrap();
        guard.insert(k, Box::new(f));
    }

    pub fn remove(&self, k: usize) {
        let mut guard = self.branches.inner.lock().unwrap();
        guard.remove(&k);
    }

    pub fn send(&self, a: &A) {
        let mut guard = self.branches.inner.lock().unwrap();
        guard.values_mut().for_each(|f| {
            f(a);
        });
    }
}

/// A pinned, possible future message.
#[cfg(target_arch = "wasm32")]
pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<A>>>>;
#[cfg(not(target_arch = "wasm32"))]
pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

//! Conditionally compiled code.
use std::{ops::Deref, collections::HashMap, future::Future, pin::Pin, sync::atomic::AtomicUsize};

/// A marker trait for messages that can be sent on a Transmitter.
#[cfg(not(target_arch = "wasm32"))]
pub trait Transmission: Send + Sync + 'static {}
#[cfg(target_arch = "wasm32")]
pub trait Transmission: 'static {}

#[cfg(not(target_arch = "wasm32"))]
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
            inner: std::Rc::new(t),
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
        f(&mut self.inner.borrow())
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

impl<A: Transmission> Responders<A> {
    pub fn insert(&self, k: usize, f: impl FnMut(&A) + Transmission) {
        self.branches.visit_mut(|b| b.insert(k, Box::new(f)));
    }

    pub fn remove(&self, k: usize) {
        self.branches.visit_mut(|b| b.remove(&k));
    }

    /// Fetch the next available responder index, incrementing it.
    pub fn get_next_k(&self) -> usize {
        self.next_k.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    pub fn send(&self, a: &A) {
        self.branches.visit_mut(|b| b.values_mut().for_each(|f| f(a)))
    }
}

/// A pinned, possible future message.
#[cfg(target_arch = "wasm32")]
pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<A>>>>;
#[cfg(not(target_arch = "wasm32"))]
pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub fn spawn<FutureA: FutureMessage<()>>(fa: FutureA) {
            wasm_bindgen_futures::spawn_local(fa);
        }
    } else if #[cfg(feature = "async-tokio")] {
        pub fn spawn<FutureA: FutureMessage<()>>(fa: FutureA) {
            let _ = tokio::task::spawn(fa);
        }
    } else if #[cfg(feature = "async-smol")] {
        pub fn spawn<FutureA: FutureMessage<()>>(fa: FutureA) {
            let _ = smol::spawn(fa);
        }
    } else {
        compile_error!("no support for async - you must build for wasm32 or enable one of the async-tokio or async-smol features");
    }
}

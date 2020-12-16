//! Data that transmits updates to subscribers automatically.
use crate::txrx::*;
use std::{cell::RefCell, rc::Rc};

/// Wraps a value `T` and transmits updates to subscribers.
#[derive(Clone)]
pub struct Model<T> {
    value: Rc<RefCell<T>>,
    trns: Transmitter<T>,
    recv: Receiver<T>,
}

impl<T: Clone + 'static> Model<T> {
    /// Create a new model from a `T`.
    pub fn new(t: T) -> Model<T> {
        let (trns, recv) = txrx::<T>();
        Model {
            value: Rc::new(RefCell::new(t)),
            trns,
            recv,
        }
    }

    /// Manually transmitter the inner value of this model to subscribers.
    pub fn transmit(&self) {
        self.trns.send(&self.value.as_ref().borrow());
    }

    /// Visit the wrapped value with a function that produces a value.
    pub fn visit<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&T) -> A,
    {
        f(&self.value.borrow())
    }

    /// Visit the mutable wrapped value with a function that produces a value.
    pub fn visit_mut<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&mut T) -> A,
    {
        let a = f(&mut self.value.borrow_mut());
        self.transmit();
        a
    }

    /// Replaces the wrapped value with a new one, returning the old value, without deinitializing either one.
    ///
    /// This function corresponds to std::mem::replace.
    pub fn replace(&self, t: T) -> T {
        let t = self.value.replace(t);
        self.transmit();
        t
    }

    /// Replaces the wrapped value with a new one computed from f, returning the old value, without deinitializing either one.
    pub fn replace_with<F>(&self, f: F) -> T
    where
        F: FnOnce(&mut T) -> T,
    {
        let t = self.value.replace_with(f);
        self.transmit();
        t
    }

    /// Access the model's receiver.
    ///
    /// The returned receiver can be used to subscribe to the model's updates.
    pub fn recv(&self) -> &Receiver<T> {
        &self.recv
    }
}

impl<T: Clone + Default + 'static> Model<T> {
    /// Takes the wrapped value, leaving Default::default() in its place.
    pub fn take(&self) -> T {
        let new_t = Default::default();
        self.replace(new_t)
    }
}

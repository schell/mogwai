//! Data that transmits updates to subscribers automatically.
use crate::{Transmitter, Receiver, channel, patch::{Patch, PatchApply}};
use std::{ops::DerefMut, sync::{Arc, Mutex}};

/// Wraps a value `T` and transmits updates to subscribers.
///
/// A `Model` may be cloned, sharing its underlying data. When any clone of a `Model` is updated
/// all downstream receivers will get a message containing the new value.
#[derive(Clone)]
pub struct Model<T> {
    value: Arc<Mutex<T>>,
    trns: Transmitter<T>,
    recv: Receiver<T>,
}

impl<T: Send + Sync + 'static> Model<T> {
    /// Create a new model from a `T`.
    pub fn new(t: T) -> Model<T> {
        let (trns, recv) = channel::<T>();
        Model {
            value: Arc::new(Mutex::new(t)),
            trns,
            recv,
        }
    }

    /// Manually send the inner value of this model to subscribers.
    fn transmit(&self) {
        let guard = self.value.lock().unwrap();
        self.trns.send(&guard);
    }

    /// Visit the wrapped value with a function that produces a value.
    pub fn visit<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&T) -> A,
    {
        let guard = self.value.lock().unwrap();
        f(&guard)
    }

    /// Visit the mutable wrapped value with a function that produces a value.
    pub fn visit_mut<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&mut T) -> A,
    {
        let mut guard = self.value.lock().unwrap();
        let a = f(&mut guard);
        self.transmit();
        a
    }

    /// Replaces the wrapped value with a new one, returning the old value, without deinitializing either one.
    ///
    /// This function corresponds to std::mem::replace.
    pub fn replace(&self, t: T) -> T {
        let mut guard = self.value.lock().unwrap();
        let prev = std::mem::replace(guard.deref_mut(), t);
        self.transmit();
        prev
    }

    /// Access the model's receiver.
    ///
    /// The returned receiver can be used to subscribe to the model's updates.
    pub fn receiver(&self) -> &Receiver<T> {
        &self.recv
    }
}

impl<T: Send + Sync + Default + 'static> Model<T> {
    /// Takes the wrapped value, leaving Default::default() in its place.
    pub fn take(&self) -> T {
        let new_t = Default::default();
        self.replace(new_t)
    }
}


/// Wraps a list of `T` values and transmits patch updates to subscribers.
///
/// A `PatchModel` may be cloned, sharing its underlying data. When any clone of a `PatchModel` is updated
/// all downstream receivers will get a message containing the update.
///
/// A `PatchModel` differs from a `Model` in that a `PatchModel` only sends the _updates_ to the inner values,
/// instead of the entire list itself. In other words the `T` in `PatchModel<T>` is just _one item_ in the list
/// of values.
pub struct PatchListModel<T> {
    value: Arc<Mutex<Vec<T>>>,
    trns: Transmitter<Patch<T>>,
    recv: Receiver<Patch<T>>,
}

impl<T: Clone + Send + Sync + 'static> PatchListModel<T> {
    /// Create a new list model from a list of `T`s.
    pub fn new<A: IntoIterator<Item = T>>(ts: A) -> PatchListModel<T> {
        let (trns, recv) = channel::<Patch<T>>();
        PatchListModel {
            value: Arc::new(Mutex::new(ts.into_iter().collect::<Vec<T>>())),
            trns,
            recv,
        }
    }

    /// Visit the wrapped values with a function that produces a value.
    pub fn visit<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&Vec<T>) -> A,
    {
        let guard = self.value.lock().unwrap();
        f(&guard)
    }

    /// Visit the value at the given index with a function that produces a value.
    pub fn visit_item<F, A>(&self, i:usize, f: F) -> A
    where
        A: 'static,
        F: FnOnce(Option<&T>) -> A,
    {
        let guard = self.value.lock().unwrap();
        f(guard.get(i))
    }

    /// Visit the values with a function that produces an update, then apply that update and send it
    /// to all downstream receivers. Return the removed items, if any.
    pub fn patch<F>(&self, f: F) -> Vec<T>
    where
        F: FnOnce(&Vec<T>) -> Option<Patch<T>>,
    {
        let mut ts = self.value.lock().unwrap();
        if let Some(update) = f(&ts) {
            let removed = ts.patch_apply(update.clone());
            self.trns.send(&update);
            removed
        } else {
            vec![]
        }
    }

    /// Access the model's receiver.
    ///
    /// The returned receiver can be used to subscribe to the model's updates.
    pub fn receiver(&self) -> &Receiver<Patch<T>> {
        &self.recv
    }
}

impl<T: Clone + Send + Sync + 'static> PatchApply for PatchListModel<T> {
    type Item = T;

    fn patch_apply(&mut self, patch: Patch<Self::Item>) -> Vec<Self::Item> {
        self.patch(|_| Some(patch))
    }
}

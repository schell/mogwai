//! Data that transmits updates to subscribers automatically.
use std::ops::Deref;

use crate::{
    channel::{channel, Counted, Shared, Transmission},
    patch::{Patch, PatchApply},
    Receiver, Transmitter,
};

/// Wraps a value `T` and transmits updates to subscribers.
///
/// A `Model` may be cloned, sharing its underlying data. When any clone of a `Model` is updated
/// all downstream receivers will get a message containing the new value.
///
/// ```rust
/// extern crate mogwai_chan;
/// use mogwai_chan::model::*;
///
/// let model_a = Model::new("hello".to_string());
/// let model_b = model_a.clone();
/// assert_eq!(model_b.visit(|s| s.clone()).as_str(), "hello");
///
/// model_b.replace("goodbye".to_string());
/// assert_eq!(model_a.visit(|s| s.clone()).as_str(), "goodbye");
/// ```
pub struct Model<T> {
    value: Counted<Shared<T>>,
    trns: Transmitter<T>,
    recv: Receiver<T>,
}

impl<T: Transmission> Clone for Model<T> {
    fn clone(&self) -> Self {
        Model {
            value: self.value.clone(),
            trns: self.trns.clone(),
            recv: self.recv.clone(),
        }
    }
}

impl<T: Clone + Transmission> Model<T> {
    /// Create a new model from a `T`.
    pub fn new(t: T) -> Model<T> {
        let (trns, recv) = channel::<T>();
        Model {
            value: Counted::new(Shared::new(t)),
            trns,
            recv,
        }
    }

    /// Manually send the inner value of this model to subscribers.
    fn transmit(&self) {
        self.value.deref().visit(|v| self.trns.send(v));
    }

    /// Visit the wrapped value with a function that produces a value.
    pub fn visit<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&T) -> A,
    {
        self.value.visit(f)
    }

    /// Visit the mutable wrapped value with a function that produces a value.
    pub fn visit_mut<F, A>(&self, f: F) -> A
    where
        A: 'static,
        F: FnOnce(&mut T) -> A,
    {
        let a = self.value.visit_mut(f);
        self.transmit();
        a
    }

    /// Replaces the wrapped value with a new one, returning the old value, without deinitializing either one.
    ///
    /// This function corresponds to std::mem::replace.
    pub fn replace(&self, t: T) -> T {
        let t1 = self.value.visit_mut(|v| std::mem::replace(v, t));
        self.transmit();
        t1
    }

    /// Replaces the wrapped value with a new one computed from f, returning the old value, without deinitializing either one.
    pub fn replace_with<F>(&self, f: F) -> T
    where
        F: FnOnce(&mut T) -> T,
    {
        let t = self.value.visit_mut(|v| {
            let t0 = f(v);
            std::mem::replace(v, t0)
        });
        self.transmit();
        t
    }

    /// Access the model's receiver.
    ///
    /// The returned receiver can be used to subscribe to the model's updates.
    pub fn receiver(&self) -> &Receiver<T> {
        &self.recv
    }

    /// Create an asynchronous stream of all this patchmodel's updates.
    pub fn updates(&self) -> impl futures::Stream<Item = T> {
        self.recv.recv_stream()
    }
}

impl<T: Clone + Default + Transmission> Model<T> {
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
    value: Counted<Shared<Vec<T>>>,
    trns: Transmitter<Patch<T>>,
    recv: Receiver<Patch<T>>,
}

impl<T: Clone + Transmission> PatchListModel<T> {
    /// Create a new list model from a list of `T`s.
    pub fn new<A: IntoIterator<Item = T>>(ts: A) -> PatchListModel<T> {
        let (trns, recv) = channel::<Patch<T>>();
        PatchListModel {
            value: Counted::new(Shared::new(ts.into_iter().collect::<Vec<T>>())),
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
        self.value.visit(f)
    }

    /// Visit the value at the given index with a function that produces a value.
    pub fn visit_item<F, A>(&self, i: usize, f: F) -> A
    where
        A: 'static,
        F: FnOnce(Option<&T>) -> A,
    {
        self.value.visit(|v| f(v.get(i)))
    }

    /// Visit the list of items with a function that produces an update, then apply that update and send it
    /// to all downstream receivers. Return the removed items, if any.
    pub fn patch<F>(&self, f: F) -> Vec<T>
    where
        F: FnOnce(&Vec<T>) -> Option<Patch<T>>,
    {
        let (may_patch, removed) = self.value.visit_mut(|vs| if let Some(update) = f(vs) {
            let removed = vs.patch_apply(update.clone());
            (Some(update), removed)
        } else {
            (None, vec![])
        });
        may_patch.iter().for_each(|u| self.trns.send(u));
        removed
    }

    /// Access the patchmodel's receiver.
    ///
    /// The returned receiver can be used to subscribe to the patchmodel's updates.
    pub fn receiver(&self) -> &Receiver<Patch<T>> {
        &self.recv
    }

    /// Create an asynchronous stream of all this patchmodel's updates.
    pub fn updates(&self) -> impl futures::Stream<Item = Patch<T>> {
        self.recv.recv_stream()
    }
}

impl<T: Clone + Transmission> PatchApply for PatchListModel<T> {
    type Item = T;

    fn patch_apply(&mut self, patch: Patch<Self::Item>) -> Vec<Self::Item> {
        self.patch(|_| Some(patch))
    }
}

#[cfg(test)]
mod test {
    use crate::new_shared;

    use super::*;

    #[test]
    fn model_sanity() {
        println!("start");
        let model_a = Model::new("hello".to_string());
        println!("created a");
        let model_b = model_a.clone();
        println!("created b");

        assert_eq!(model_b.visit(|s| s.clone()).as_str(), "hello");
        println!("visited");

        model_b.replace("goodbye".to_string());
        println!("replaced");
    }

    #[test]
    fn patchlist_sanity() {
        let mut list = PatchListModel::new(vec![]);
        list.patch_push(0);
        assert!(list.visit(|v| *v.get(0).unwrap() == 0));

        list.patch_insert(0, 1);
        assert_eq!(list.visit(Vec::clone), vec![1, 0]);

        let val = new_shared(false);
        list.receiver().branch().respond_shared(val.clone(), |v, _| {
            *v = true;
        });
        let i = list.patch_pop();
        assert_eq!(i, Some(0));
        assert!(val.visit(|b| *b));
    }
}

//! Shared data with update streams.
//!
//! A [`Model`] is a wrapper that streams updates to
//! logic loops and views. Use [`Model::stream`] to
//! access a stream of updates to the model.
//!
use futures::lock::{Mutex, MutexGuard};
use std::sync::{atomic::AtomicU32, Arc};

use crate::channel::{bounded, unbounded, Receiver, Sender, SinkExt, Stream};

/// Wraps a value `T` and provides a stream of updates.
///
/// A `Model` may be cloned, sharing its underlying data. When any clone of
/// a `Model` is mutated all observers downstream will get a message containing
/// the new value.
///
///
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
    value: Arc<Mutex<T>>,
    next_k: Arc<AtomicU32>,
    downstream: (Sender<T>, Receiver<T>),
}

impl<T: Clone + Unpin + 'static> Model<T> {
    /// Create a new model from a `T`.
    pub fn new(t: T, stream_cap: Option<usize>) -> Model<T> {
        let downstream = if let Some(cap) = stream_cap {
            bounded::<T>(cap)
        } else {
            unbounded::<T>()
        };

        let mut tx = downstream.0.clone();
        let t1 = t.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = tx.send(t1).await.unwrap();
        });

        Model {
            value: Arc::new(Mutex::new(t)),
            next_k: Arc::new(AtomicU32::new(0)),
            downstream,
        }
    }
    /// Mutably access the underlying data. When the lock goes out of scope,
    /// the new value will be sent downstream.
    ///
    /// # Fairness
    ///
    /// [`Model`] provides no fairness guarantees. Tasks may not acquire the lock
    /// in the order that they requested the lock, and it's possible for a single task
    /// which repeatedly takes the lock to starve other tasks, which may be left waiting
    /// indefinitely.
    ///
    /// For this reason you should not rely on receiving every update, only the most recent.
    pub async fn lock<'a>(&'a self) -> MutexGuard<'a, T> {
        let value = self.value.clone();
        let t = self.value.lock().await;
        let k = self
            .next_k
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let next_k = self.next_k.clone();
        let mut tx = self.downstream.0.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let lock = value.lock().await;
            let k1: u32 = next_k.load(std::sync::atomic::Ordering::SeqCst);
            if k1 - k <= 1 {
                tx.send(lock.clone()).await.unwrap();
            }
        });
        t
    }

    /// Produce a stream of updates.
    ///
    /// You should not rely on receiving every update, only the most recent.
    /// When a task mutates the model by calling [`Model::lock`], an update
    /// is scheduled to be sent on the stream. If mutations happen in a fast
    /// succession, previous sends will be cancelled.
    pub fn stream(&self) -> impl Stream<Item = T> {
        self.downstream.1.clone()
    }
}

///// Wraps a list of `T` values and transmits patch updates to subscribers.
/////
///// A `PatchModel` may be cloned, sharing its underlying data. When any clone of a `PatchModel` is updated
///// all downstream receivers will get a message containing the update.
/////
///// A `PatchModel` differs from a `Model` in that a `PatchModel` only sends the _updates_ to the inner values,
///// instead of the entire list itself. In other words the `T` in `PatchModel<T>` is just _one item_ in the list
///// of values.
//pub struct ListPatchModel<T> {
//    value: Counted<Shared<Vec<T>>>,
//    upstream: ()
//}
//
//impl<T: Clone + Transmission> ListPatchModel<T> {
//    /// Create a new list model from a list of `T`s.
//    pub fn new<A: IntoIterator<Item = T>>(ts: A) -> ListPatchModel<T> {
//        let (trns, recv) = channel::<Patch<T>>();
//        ListPatchModel {
//            value: Counted::new(Shared::new(ts.into_iter().collect::<Vec<T>>())),
//            trns,
//            recv,
//        }
//    }
//
//    /// Visit the wrapped values with a function that produces a value.
//    pub fn visit<F, A>(&self, f: F) -> A
//    where
//        A: 'static,
//        F: FnOnce(&Vec<T>) -> A,
//    {
//        self.value.visit(f)
//    }
//
//    /// Visit the value at the given index with a function that produces a value.
//    pub fn visit_item<F, A>(&self, i: usize, f: F) -> A
//    where
//        A: 'static,
//        F: FnOnce(Option<&T>) -> A,
//    {
//        self.value.visit(|v| f(v.get(i)))
//    }
//
//    /// Visit the list of items with a function that produces an update, then apply that update and send it
//    /// to all downstream receivers. Return the removed items, if any.
//    pub fn patch<F>(&self, f: F) -> Vec<T>
//    where
//        F: FnOnce(&Vec<T>) -> Option<Patch<T>>,
//    {
//        let (may_patch, removed) = self.value.visit_mut(|vs| if let Some(update) = f(vs) {
//            let removed = vs.patch_apply(update.clone());
//            (Some(update), removed)
//        } else {
//            (None, vec![])
//        });
//        may_patch.iter().for_each(|u| self.trns.send(u));
//        removed
//    }
//
//    /// Access the patchmodel's receiver.
//    ///
//    /// The returned receiver can be used to subscribe to the patchmodel's updates.
//    pub fn receiver(&self) -> &Receiver<Patch<T>> {
//        &self.recv
//    }
//
//    /// Create an asynchronous stream of all this patchmodel's updates.
//    pub fn updates(&self) -> impl futures::Stream<Item = Patch<T>> {
//        self.recv.recv_stream()
//    }
//}
//
//impl<T: Clone + Transmission> PatchApply for ListPatchModel<T> {
//    type Item = T;
//
//    fn patch_apply(&mut self, patch: Patch<Self::Item>) -> Vec<Self::Item> {
//        self.patch(|_| Some(patch))
//    }
//}
//
#[cfg(test)]
mod test {
    use super::*;
    use futures::StreamExt;
    use wasm_bindgen_test::*;
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn model_sanity() {
        console_log::init_with_level(log::Level::Trace).unwrap();

        let model = Model::new("hello".to_string(), None);
        {
            // This mutation's send is clobbered because the next
            // mutation happens before this update's send
            let mut lock = model.lock().await;
            *lock = "hi".to_string();
        }

        {
            let mut lock = model.lock().await;
            *lock = "goodbye".to_string();
        }

        let mut stream = model.stream();
        assert_eq!(stream.next().await.unwrap().as_str(), "hello");
        assert_eq!(stream.next().await.unwrap().as_str(), "goodbye");
    }
    //
    //    #[test]
    //    fn patchlist_sanity() {
    //        let mut list = ListPatchModel::new(vec![]);
    //        list.patch_push(0);
    //        assert!(list.visit(|v| *v.get(0).unwrap() == 0));
    //
    //        list.patch_insert(0, 1);
    //        assert_eq!(list.visit(Vec::clone), vec![1, 0]);
    //
    //        let val = new_shared(false);
    //        list.receiver().branch().respond_shared(val.clone(), |v, _| {
    //            *v = true;
    //        });
    //        let i = list.patch_pop();
    //        assert_eq!(i, Some(0));
    //        assert!(val.visit(|b| *b));
    //    }
}

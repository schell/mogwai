//! Values with streams of updates.
use std::{collections::HashMap, ops::DerefMut, sync::Arc};

use async_broadcast::{broadcast, Receiver, Sender};
use futures::Stream;
use async_lock::{RwLock, RwLockReadGuard};

use crate::{
    patch::{HashPatch, ListPatch},
    target::{Sendable, Syncable},
};

pub use crate::patch::{HashPatchApply, ListPatchApply};

/// Wraps a value `T` and provides a stream of the latest value.
///
/// [`Model`] can be easily shared for mutual mutation by cloning,
/// or can be used to stream updated values to observers.
///
/// ## Warning
/// If [`Model::write`] is called in quick succession, only the
/// latest, unique values will be sent to downstream observers.
///
/// ```rust
/// use mogwai::model::*;
/// use mogwai::futures::StreamExt;
///
/// smol::block_on(async {
///     let model_a = Model::new("hello".to_string());
///     let updates = model_a.stream();
///     model_a.visit_mut(|t| *t = "hi".to_string()).await;
///
///     let model_b = model_a.clone();
///     assert_eq!(model_b.read().await.as_str(), "hi");
///
///     model_b.visit_mut(|t| *t = "goodbye".to_string()).await;
///     assert_eq!(model_a.read().await.as_str(), "goodbye");
///     drop(model_b);
///
///     drop(model_a);
///
///     assert_eq!(updates.collect::<Vec<_>>().await, vec!["goodbye".to_string()]);
/// });
/// ```
pub struct Model<T> {
    value: Arc<RwLock<T>>,
    chan: (Sender<T>, Receiver<T>),
}

impl<T> std::fmt::Debug for Model<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Model<{}>", std::any::type_name::<T>())).finish()
    }
}

impl<T> Clone for Model<T> {
    fn clone(&self) -> Self {
        Model {
            value: self.value.clone(),
            chan: self.chan.clone(),
        }
    }
}

impl<T: Clone + Sendable + Syncable + PartialEq> Model<T> {
    /// Create a new Model.
    pub fn new(t: T) -> Model<T> {
        let (mut tx, rx) = broadcast::<T>(1);
        tx.set_overflow(true);
        tx.try_broadcast(t.clone()).unwrap();

        Model {
            value: Arc::new(RwLock::new(t)),
            chan: (tx, rx),
        }
    }

    /// Acquires a read lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Note that attempts to acquire a read lock will block if there are also
    /// concurrent attempts to acquire a write lock.
    pub async fn read<'a>(&'a self) -> RwLockReadGuard<'a, T> {
        self.value.read().await
    }

    /// Visits the inner value of the model mutably. After the closure returns
    /// the inner value will be sent to all downstream observers.
    pub async fn visit_mut<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        let mut lock = self.value.write().await;
        f(lock.deref_mut());
        let t = lock.clone();
        self.chan.0.broadcast(t).await.unwrap();
    }

    /// Produce a stream of updated values.
    ///
    /// You should not rely on receiving every update, only the most recent.
    /// When a task mutates the model by calling [`Model::write`], an update
    /// is scheduled to be sent on the stream. If mutations happen in a fast
    /// succession, previous sends will be clobbered.
    pub fn stream(&self) -> impl Stream<Item = T> {
        self.chan.1.clone()
    }
}

/// Provides a patchable list of `T` and a stream of patch updates.
///
/// [`ListPatchModel`] is great for synchronizing two isomorphorphic
/// data structures - such as a list of strings and a list of DOM elements.
///
/// [`ListPatchModel::stream`] operates much the same as [`Model::stream`],
/// but instead of sending new updated values of `T` downstream,
/// [`ListPatchModel`] sends patches that can be applied to isomorphic structures.
///
/// Unlike [`Model`], downstream observers are guaranteed to receive a message of
/// every patch applied to the model.
///
/// Unlike [`Model`], [`ListPatchModel`] is not meant to be shared by cloning.
pub struct ListPatchModel<T> {
    value: RwLock<Vec<T>>,
    chan: (Sender<ListPatch<T>>, Receiver<ListPatch<T>>),
}

impl<T: Clone> ListPatchModel<T> {
    /// Create a new, empty ListPatchModel.
    pub fn new() -> Self {
        let downstream = broadcast::<ListPatch<T>>(4);
        ListPatchModel {
            value: Default::default(),
            chan: downstream,
        }
    }

    /// Acquires a read lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Note that attempts to acquire a read lock will block if there are also
    /// concurrent attempts to acquire a write lock.
    pub async fn read<'a>(&'a self) -> RwLockReadGuard<'a, Vec<T>> {
        self.value.read().await
    }

    /// Produce a stream of updates.
    ///
    /// Guaranteed to receive a clone of every patch.
    pub fn stream(&self) -> impl Stream<Item = ListPatch<T>> {
        self.chan.1.clone()
    }
}

impl<T: Clone> ListPatchApply for ListPatchModel<T> {
    type Item = T;

    /// Apply the given patch to the `ListPatchModel`.
    ///
    /// Blocks until a write lock can be acquired.
    ///
    /// ## Panics
    /// Panics if the downstream channel is full.
    fn list_patch_apply(&mut self, patch: ListPatch<Self::Item>) -> Vec<Self::Item> {
        let items = self.value.get_mut().list_patch_apply(patch.clone());
        let tx = &mut self.chan.0;
        tx.set_capacity(1 + tx.len());
        let _ = tx.try_broadcast(patch).unwrap();
        items
    }
}

/// Wraps a collection of key value pairs and provides a stream of
/// [`HashPatch<K, V>`] updates.
///
/// Much like [`Model`] but instead of sending new updated
/// values of `T` downstream, [`HashPatchModel`] sends patches
/// that can be applied to isomorphic structures.
///
/// Unlike [`Model`], downstream observers are guaranteed a
/// message of every patch applied to the model.
///
/// ```rust
/// use mogwai::model::*;
/// smol::block_on(async {
///     let mut model: HashPatchModel<String, usize> = HashPatchModel::new();
///     model.hash_patch_insert("hello".to_string(), 666);
///     assert_eq!(model.read().await.get("hello"), Some(&666));
/// });
/// ```
pub struct HashPatchModel<K, V> {
    value: RwLock<HashMap<K, V>>,
    chan: (Sender<HashPatch<K, V>>, Receiver<HashPatch<K, V>>),
}

impl<K: Clone, V: Clone> HashPatchModel<K, V> {
    /// Create a new HashPatchModel.
    pub fn new() -> Self {
        let chan = broadcast::<HashPatch<K, V>>(4);
        HashPatchModel {
            value: Default::default(),
            chan,
        }
    }

    /// Acquires a read lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Note that attempts to acquire a read lock will block if there are also
    /// concurrent attempts to acquire a write lock.
    pub async fn read<'a>(&'a self) -> RwLockReadGuard<'a, HashMap<K, V>> {
        self.value.read().await
    }

    /// Produce a stream of updates.
    ///
    /// Guaranteed to receive a clone of every patch.
    pub fn stream(&self) -> impl Stream<Item = HashPatch<K, V>> {
        self.chan.1.clone()
    }
}

impl<K: Clone + std::hash::Hash + Eq, V: Clone> HashPatchApply for HashPatchModel<K, V> {
    type Key = K;
    type Value = V;

    /// Apply the given patch to the `HashPatchModel`.
    ///
    /// Blocks until all downstream observers have received the patch.
    fn hash_patch_apply(
        &mut self,
        patch: HashPatch<Self::Key, Self::Value>,
    ) -> Option<Self::Value> {
        let item = self.value.get_mut().hash_patch_apply(patch.clone());
        let tx = &mut self.chan.0;
        tx.set_capacity(1 + tx.len());
        let _ = tx.try_broadcast(patch).unwrap();
        item
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn model_sanity() {
        let model = Model::new("hello".to_string());
        let stream = model.stream();
        futures::executor::block_on(async move {
            model.visit_mut(|t| *t = "hi".to_string()).await;
            model.visit_mut(|t| *t = "goodbye".to_string()).await;
            drop(model);

            assert_eq!(
                stream.collect::<Vec<_>>().await,
                vec!["goodbye".to_string()]
            );
        });
    }

    #[test]
    fn list_patch_model_sanity() {
        let mut model: ListPatchModel<String> = ListPatchModel::new();
        let stream = model.stream();
        futures::executor::block_on(async move {
            model.list_patch_push("hello".to_string());
            model.list_patch_push("hi".to_string());
            model.list_patch_push("goodbye".to_string());
            drop(model);

            let mut iso: Vec<String> = vec![];
            for patch in stream.collect::<Vec<_>>().await {
                iso.list_patch_apply(patch);
            }

            assert_eq!(
                iso,
                vec!["hello".to_string(), "hi".to_string(), "goodbye".to_string()]
            );
        });
    }

    #[test]
    fn hash_patch_model_sanity() {
        let mut model: HashPatchModel<String, usize> = HashPatchModel::new();
        let stream = model.stream();
        futures::executor::block_on(async move {
            model.hash_patch_insert("zero".to_string(), 0);
            model.hash_patch_insert("two".to_string(), 2);
            model.hash_patch_insert("one".to_string(), 1);
            drop(model);

            let mut iso: Vec<(String, usize)> = vec![];
            for patch in stream.collect::<Vec<_>>().await {
                iso.hash_patch_apply(patch);
            }

            assert_eq!(
                iso,
                vec![
                    ("zero".to_string(), 0),
                    ("two".to_string(), 2),
                    ("one".to_string(), 1)
                ]
            );
        });
    }

    //#[test]
    //fn channel_sanity() {
    //    let (tx, rx1) = async_channel::unbounded::<u32>();
    //    let rx2 = rx1.clone();
    //    let t1 = smol::spawn(async move {
    //        let n = rx2.recv().await.unwrap();
    //        assert_eq!(n, 666);
    //    });
    //    let t2 =
    //        smol::spawn(async move {
    //        let n = rx1.recv().await.unwrap();
    //        assert_eq!(n, 666);
    //    });

    //    smol::block_on(async move {
    //        tx.send(666).await.unwrap();
    //        tx.send(123).await.unwrap();
    //        let ((), ()) = futures::future::join(t1, t2).await;
    //    });
    //}
}

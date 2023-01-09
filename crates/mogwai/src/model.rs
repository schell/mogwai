//! Values with streams of updates.
use std::{
    collections::HashMap,
    ops::{DerefMut, RangeBounds},
    sync::Arc,
};

use anyhow::Context;
use async_broadcast::{broadcast, Receiver, Sender};
use async_lock::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard};

pub use crate::patch::{HashPatchApply, ListPatchApply};
use crate::{
    patch::{HashPatch, ListPatch},
    stream::Stream,
};

/// Wraps a value `T` and provides a stream of the latest value.
///
/// [`Model`] can be easily shared for mutual mutation by cloning,
/// or can be used to stream updated values to observers.
///
/// ## Warning
/// If [`Model::visit_mut`] is called in quick succession, only the
/// latest, unique values will be sent to downstream observers.
///
/// ```rust
/// use mogwai::{model::Model, prelude::*};
///
/// mogwai::future::block_on(async {
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
///     assert_eq!(
///         updates.collect::<Vec<_>>().await,
///         vec!["goodbye".to_string()]
///     );
/// });
/// ```
pub struct Model<T> {
    value: Arc<RwLock<T>>,
    chan: (Sender<T>, Receiver<T>),
}

impl<T> std::fmt::Debug for Model<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Model<{}>", std::any::type_name::<T>()))
            .finish()
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

impl<T: Clone + PartialEq> Model<T> {
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

    /// Attempt to get a clone of the current inner value.
    ///
    /// This will fail if the model is actively being mutated.
    pub fn current(&self) -> Option<T> {
        let lock = self.value.try_read();
        lock.as_deref().cloned()
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
    /// When a task mutates the model by calling [`Model::visit_mut`], an update
    /// is scheduled to be sent on the stream. If mutations happen in a fast
    /// succession, previous sends will be clobbered.
    pub fn stream(&self) -> impl Stream<Item = T> {
        self.chan.1.clone()
    }
}

/// Provides a patchable list of `T` and a stream of patch updates.
///
/// [`ListPatchModel`] is great for synchronizing two or more list structures -
/// such as a list of strings and a list of DOM elements.
///
/// [`ListPatchModel::stream`] operates much the same as [`Model::stream`],
/// but instead of sending new updated values of `T` downstream,
/// [`ListPatchModel`] sends patches downstream.
///
/// Unlike [`Model`], downstream observers are guaranteed to receive a message
/// of every patch applied to the model after subscription.
#[derive(Clone)]
pub struct ListPatchModel<T> {
    value: Arc<RwLock<Vec<T>>>,
    chan: (Arc<RwLock<Sender<ListPatch<T>>>>, Receiver<ListPatch<T>>),
}

impl<T> Default for ListPatchModel<T> {
    fn default() -> Self {
        let (tx, rx) = broadcast::<ListPatch<T>>(4);
        ListPatchModel {
            value: Default::default(),
            chan: (Arc::new(RwLock::new(tx)), rx),
        }
    }
}

impl<T: Clone> ListPatchModel<T> {
    /// Create a new, empty ListPatchModel.
    pub fn new() -> Self {
        Self::default()
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

    /// Applies the function to the inner value.
    pub async fn visit<X>(&self, f: impl FnOnce(&Vec<T>) -> X) -> X {
        f(self.read().await.as_ref())
    }

    /// Applies the function to the inner value, without waiting to acquire a
    /// lock, if possible.
    pub fn try_visit<X>(&self, f: impl FnOnce(&Vec<T>) -> X) -> Option<X> {
        Some(f(self.value.try_read()?.as_ref()))
    }

    /// Produce a stream of updates.
    ///
    /// Guaranteed to receive a clone of every patch.
    pub fn stream(&self) -> impl Stream<Item = ListPatch<T>> {
        self.chan.1.clone()
    }

    async fn ensure_room(&self) {
        let tx = self.chan.0.upgradable_read().await;
        let len = tx.len();
        if tx.is_full() {
            RwLockUpgradableReadGuard::upgrade(tx)
                .await
                .set_capacity(1 + len);
        }
    }

    fn try_ensure_room(&self) -> anyhow::Result<()> {
        let tx = self
            .chan
            .0
            .try_upgradable_read()
            .context("cannot get upgradable read")?;
        let len = tx.len();
        if tx.is_full() {
            RwLockUpgradableReadGuard::try_upgrade(tx)
                .ok()
                .context("cannot upgrade read")?
                .set_capacity(1 + len);
        }

        Ok(())
    }

    /// Apply the given patch to the `ListPatchModel`, awaiting the acquisition
    /// of locks.
    pub async fn patch(&self, patch: ListPatch<T>) -> anyhow::Result<Vec<T>> {
        self.ensure_room().await;
        let tx = self.chan.0.read().await;
        let items = self.value.write().await.list_patch_apply(patch.clone());
        let _ = tx.try_broadcast(patch).ok().context("cannot broadcast")?;
        Ok(items)
    }

    /// Apply the given patch to the `ListPatchModel`.
    pub fn try_patch(&self, patch: ListPatch<T>) -> anyhow::Result<Vec<T>> {
        self.try_ensure_room()?;
        let tx = self.chan.0.try_read().context("cannot read")?;
        let items = self
            .value
            .try_write()
            .context("cannot write")?
            .list_patch_apply(patch.clone());
        let _ = tx.try_broadcast(patch).ok().context("cannot broadcast")?;
        Ok(items)
    }

    /// Force a refresh, sending a `ListPatch::Noop` downstream.
    ///
    /// This is useful when downstream structures share the same list-shape but
    /// differ in details that may have changed.
    pub async fn refresh(&self) {
        let _ = self.patch(ListPatch::Noop).await;
    }

    /// Splices the given range with the given replacements.
    ///
    /// Returns any removed items.
    pub async fn splice(
        &self,
        range: impl RangeBounds<usize>,
        replace_with: impl IntoIterator<Item = T>,
    ) -> anyhow::Result<Vec<T>> {
        self.patch(ListPatch::splice(range, replace_with)).await
    }

    /// Inserts the item at the given index.
    pub async fn insert(&self, index: usize, item: T) -> anyhow::Result<()> {
        let _ = self
            .patch(ListPatch::splice(index..index, vec![item]))
            .await?;
        Ok(())
    }

    /// Removes the item at the given index, returning it if possible.
    pub async fn remove(&self, index: usize) -> anyhow::Result<T> {
        let mut removed = self.patch(ListPatch::remove(index)).await?;
        removed
            .pop()
            .with_context(|| format!("item at index {} was not found", index))
    }

    /// Replaces the given index with the given item.
    ///
    /// Returns the item replaced, if possible.
    pub async fn replace(&self, index: usize, item: T) -> anyhow::Result<T> {
        let mut removed = self.patch(ListPatch::replace(index, item)).await?;
        removed
            .pop()
            .with_context(|| format!("item at index {} was not found", index))
    }

    /// Pushes the given item onto the end of the list.
    pub async fn push(&self, item: T) -> anyhow::Result<()> {
        self.patch(ListPatch::Push(item)).await.map(|_| ())
    }

    /// Pops the last item off the list, if possible.
    pub async fn pop(&self) -> anyhow::Result<Option<T>> {
        let mut removed = self.patch(ListPatch::Pop).await?;
        Ok(removed.pop())
    }

    /// Drains/removes the entire list.
    pub async fn drain(&self) -> anyhow::Result<Vec<T>> {
        self.patch(ListPatch::drain()).await
    }
}

impl<T: Clone> ListPatchApply for ListPatchModel<T> {
    type Item = T;

    /// Apply the given patch to the `ListPatchModel`.
    ///
    /// ## Panics
    /// Panics if the downstream channel is full, or the model is being read at
    /// the time of application (which cannot happen in the browser).
    fn list_patch_apply(&mut self, patch: ListPatch<Self::Item>) -> Vec<Self::Item> {
        self.try_patch(patch).unwrap()
    }
}

/// Wraps a collection of key value pairs and provides a stream of
/// [`HashPatch<K, V>`] updates.
///
/// Much like [`Model`] but instead of sending new updated
/// values of `T` downstream, [`HashPatchModel`] sends patches
/// that can be applied to downstream structures.
///
/// Unlike [`Model`], downstream observers are guaranteed a
/// message of every patch applied to the model.
///
/// ```rust
/// use mogwai::{model::HashPatchModel, prelude::*};
/// mogwai::future::block_on(async {
///     let mut model: HashPatchModel<String, usize> = HashPatchModel::new();
///     model.hash_patch_insert("hello".to_string(), 666);
///     assert_eq!(model.read().await.get("hello"), Some(&666));
/// });
/// ```
#[derive(Clone)]
pub struct HashPatchModel<K, V> {
    value: Arc<RwLock<HashMap<K, V>>>,
    chan: (Sender<HashPatch<K, V>>, Receiver<HashPatch<K, V>>),
}

impl<K, V> Default for HashPatchModel<K, V> {
    fn default() -> Self {
        let chan = broadcast::<HashPatch<K, V>>(4);
        HashPatchModel {
            value: Default::default(),
            chan,
        }
    }
}

impl<K: Clone, V: Clone> HashPatchModel<K, V> {
    /// Create a new HashPatchModel.
    pub fn new() -> Self {
        Self::default()
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
    /// ## Panics
    /// Panics if the downstream channel is full, or the model is being read at
    /// the time of application (which cannot happen in the browser).
    fn hash_patch_apply(
        &mut self,
        patch: HashPatch<Self::Key, Self::Value>,
    ) -> Option<Self::Value> {
        let item = self
            .value
            .try_write()
            .unwrap()
            .hash_patch_apply(patch.clone());
        let tx = &mut self.chan.0;
        tx.set_capacity(1 + tx.len());
        let _ = tx.try_broadcast(patch).unwrap();
        item
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::stream::StreamExt;

    #[test]
    fn model_sanity() {
        let model = Model::new("hello".to_string());
        let stream = model.stream();
        futures_lite::future::block_on(async move {
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
        futures_lite::future::block_on(async move {
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
        futures_lite::future::block_on(async move {
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

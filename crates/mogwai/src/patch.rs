//! Updates to lists and hashmaps encoded as enums.
use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Bound, RangeBounds},
};

use futures::Future;

fn clone_bound<T: Copy>(bound: Bound<&T>) -> Bound<T> {
    match bound {
        Bound::Included(b) => Bound::Included(*b),
        Bound::Excluded(b) => Bound::Excluded(*b),
        Bound::Unbounded => Bound::Unbounded,
    }
}

/// Variants used to patch the items in a list.
#[derive(Clone, Debug)]
pub enum ListPatch<T> {
    /// Replace the specified range in the list with the given `replace_with` items.
    /// Zero-indexed.
    Splice {
        /// The index to insert the item.
        range: (Bound<usize>, Bound<usize>),
        /// The items.
        replace_with: Vec<T>,
    },

    /// Push an item onto the back of the list of items.
    Push(T),

    /// Remove the last item.
    Pop,
}

impl<T> ListPatch<T> {
    /// Construct a ListPatch that splices the given range with the given replacements.
    pub fn splice(range: impl RangeBounds<usize>, replace_with: impl Iterator<Item = T>) -> Self {
        ListPatch::Splice {
            range: (
                clone_bound(range.start_bound()),
                clone_bound(range.end_bound()),
            ),
            replace_with: replace_with.collect(),
        }
    }

    /// Construct a ListPatch that replaces the given index with the given item.
    pub fn replace(index: usize, item: T) -> Self {
        Self::splice(index..=index, std::iter::once(item))
    }

    /// Construct a ListPatch that pushes the given item onto the end of the list.
    pub fn push(item: T) -> Self {
        ListPatch::Push(item)
    }

    /// Construct a ListPatch that pops the last item.
    pub fn pop() -> Self {
        ListPatch::Pop
    }

    /// Construct a ListPatch that drains/removes the entire list.
    pub fn drain() -> Self {
        ListPatch::splice(.., std::iter::empty())
    }

    /// Map the patch from `T` to `X`
    pub fn map<F, X>(self, f: F) -> ListPatch<X>
    where
        F: Fn(T) -> X,
    {
        match self {
            ListPatch::Splice {
                range,
                replace_with,
            } => ListPatch::Splice {
                range,
                replace_with: replace_with.into_iter().map(f).collect::<Vec<_>>(),
            },
            ListPatch::Push(value) => ListPatch::Push(f(value)),
            ListPatch::Pop => ListPatch::Pop,
        }
    }

    /// Map the patch from `T` to `X` using a function that returns a future that produces
    /// an `X`.
    pub async fn map_future<F, X, Fut>(self, f: F) -> ListPatch<X>
    where
        F: Fn(T) -> Fut,
        Fut: Future<Output = X>
    {
        match self {
            ListPatch::Splice {
                range,
                replace_with,
            } => ListPatch::Splice {
                range,
                replace_with: futures::future::join_all(replace_with.into_iter().map(f)).await,
            },
            ListPatch::Push(value) => ListPatch::Push(f(value).await),
            ListPatch::Pop => ListPatch::Pop,
        }
    }
}

/// Provides `list_patch_apply` (and friends) to list types.
pub trait ListPatchApply {
    /// The underlying item type of the list being patched.
    type Item;

    /// Apply the given patch, modifying the list and returning the removed items.
    fn list_patch_apply(&mut self, patch: ListPatch<Self::Item>) -> Vec<Self::Item>;

    /// Insert the given item into the list at the given index, pushing all other items to the right.
    fn list_patch_insert(&mut self, index: usize, item: Self::Item) {
        let ts = self.list_patch_splice(index..index, vec![item]);
        assert!(ts.is_empty());
    }

    /// Swap the item at the given index with the given item. Return the original item, if possible.
    fn list_patch_swap(&mut self, index: usize, item: Self::Item) -> Option<Self::Item> {
        let mut ts = self.list_patch_splice(index..=index, vec![item]);
        assert!(ts.len() <= 1, "unexpected number of removed items");
        match ts.len() {
            0 => None,
            1 => ts.pop(),
            _ => unreachable!(),
        }
    }

    /// Remove the item at the give index. Return the original item, if possible.
    fn list_patch_remove(&mut self, index: usize) -> Option<Self::Item> {
        let mut ts = self.list_patch_splice(index..=index, vec![]);
        assert!(ts.len() <= 1, "unexpected number of removed items");
        match ts.len() {
            0 => None,
            1 => ts.pop(),
            _ => unreachable!(),
        }
    }

    /// Pushes the item to the end of the list.
    fn list_patch_push(&mut self, item: Self::Item) {
        let ts = self.list_patch_apply(ListPatch::Push(item));
        assert!(ts.is_empty());
    }

    /// Removes the last item and returns it, if possible.
    fn list_patch_pop(&mut self) -> Option<Self::Item> {
        let mut ts = self.list_patch_apply(ListPatch::Pop);
        assert!(ts.len() <= 1);
        ts.pop()
    }

    /// Replace the specified range in the list with the given `replace_with` items.
    /// Returns any removed items.
    fn list_patch_splice<R: RangeBounds<usize>, I: IntoIterator<Item = Self::Item>>(
        &mut self,
        range: R,
        replace_with: I,
    ) -> Vec<Self::Item> {
        let range = (
            clone_bound(range.start_bound()),
            clone_bound(range.end_bound()),
        );
        let replace_with = replace_with.into_iter().collect::<Vec<_>>();
        self.list_patch_apply(ListPatch::Splice {
            range,
            replace_with,
        })
    }
}

impl<T> ListPatchApply for Vec<T> {
    type Item = T;

    fn list_patch_apply(&mut self, patch: ListPatch<T>) -> Vec<T> {
        match patch {
            ListPatch::Splice {
                range,
                replace_with,
            } => self.splice(range, replace_with).collect::<Vec<T>>(),
            ListPatch::Push(value) => {
                self.push(value);
                vec![]
            }
            ListPatch::Pop => self.pop().map(|t| vec![t]).unwrap_or_else(|| vec![]),
        }
    }
}

impl ListPatchApply for web_sys::Node {
    type Item = web_sys::Node;

    fn list_patch_apply(&mut self, patch: ListPatch<Self::Item>) -> Vec<Self::Item> {
        let mut removed = vec![];
        match patch {
            crate::patch::ListPatch::Splice {
                range,
                replace_with,
            } => {
                let mut replace_with = replace_with.into_iter();
                let list: web_sys::NodeList = self.child_nodes();
                let children: Vec<web_sys::Node> =
                    (0..list.length()).filter_map(|i| list.get(i)).collect();

                let start_index = match range.0 {
                    Bound::Included(i) => i,
                    Bound::Excluded(i) => i,
                    Bound::Unbounded => 0,
                };
                let end_index = match range.1 {
                    Bound::Included(i) => i,
                    Bound::Excluded(i) => i,
                    Bound::Unbounded => (list.length() as usize).max(1) - 1,
                };

                let mut child_after = None;
                for i in start_index..=end_index {
                    if let Some(old_child) = children.get(i) {
                        if range.contains(&i) {
                            if let Some(new_child) = replace_with.next() {
                                self.replace_child(&new_child, &old_child).unwrap();
                            } else {
                                self.remove_child(&old_child).unwrap();
                            }
                            removed.push(old_child.clone());
                        } else {
                            child_after = Some(old_child);
                        }
                    }
                }

                for child in replace_with {
                    self.insert_before(&child, child_after).unwrap();
                }
            }
            crate::patch::ListPatch::Push(new_node) => {
                let _ = self.append_child(&new_node).unwrap();
            }
            crate::patch::ListPatch::Pop => {
                if let Some(child) = self.last_child() {
                    let _ = self.remove_child(&child).unwrap();
                    removed.push(child);
                }
            }
        }
        removed
    }
}

#[cfg(test)]
mod list {
    use super::*;

    #[test]
    fn splice_sanity() {
        let mut vs = vec![0, 1, 2];
        let is = vs.splice(0..0, vec![3]).collect::<Vec<_>>();
        assert!(is.is_empty());
        assert_eq!(vs, vec![3, 0, 1, 2]);
    }

    #[test]
    fn range_sanity() {
        let range = 0..0;
        assert!(!range.contains(&0));
    }

    #[test]
    fn vec_patching() {
        let mut vs = vec![0, 1, 2, 3, 4, 5];

        vs.list_patch_insert(2, 666);
        assert_eq!(&vs, &[0, 1, 666, 2, 3, 4, 5]);

        vs.list_patch_swap(2, 0xC0FFEE);
        assert_eq!(&vs, &[0, 1, 0xC0FFEE, 2, 3, 4, 5]);

        vs.list_patch_remove(2);
        assert_eq!(&vs, &[0, 1, 2, 3, 4, 5]);

        let _ = vs.list_patch_splice(0.., vec![]);
        assert!(&vs.is_empty());

        vs.list_patch_push(0);
        vs.list_patch_push(1);
        assert_eq!(&vs, &[0, 1]);

        let n = vs.list_patch_pop().unwrap();
        assert_eq!(n, 1);
        assert_eq!(&vs, &[0]);
    }
}

/// Variants used to patch the items in a hash map.
#[derive(Clone, Debug, PartialEq)]
pub enum HashPatch<K, V> {
    /// Insert value `V` at key `K`
    Insert(K, V),
    /// Remove the value at `K`
    Remove(K),
}

/// Provides `hash_patch_apply`
pub trait HashPatchApply {
    /// Key type of the hash map being patched.
    type Key;
    /// Value type of the hash map being patched.
    type Value;

    /// Apply a patch to a hash map.
    fn hash_patch_apply(&mut self, patch: HashPatch<Self::Key, Self::Value>)
        -> Option<Self::Value>;

    /// Insert.
    fn hash_patch_insert(&mut self, k: Self::Key, v: Self::Value) -> Option<Self::Value> {
        self.hash_patch_apply(HashPatch::Insert(k, v))
    }

    /// Get.
    fn hash_patch_remove(&mut self, k: Self::Key) -> Option<Self::Value> {
        self.hash_patch_apply(HashPatch::Remove(k))
    }
}

impl<K, V> HashPatchApply for HashMap<K, V>
where
    K: Hash + Eq,
{
    type Key = K;
    type Value = V;

    fn hash_patch_apply(
        &mut self,
        patch: HashPatch<Self::Key, Self::Value>,
    ) -> Option<Self::Value> {
        match patch {
            HashPatch::Insert(k, v) => self.insert(k, v),
            HashPatch::Remove(k) => self.remove(&k),
        }
    }
}

impl<K, V> HashPatchApply for Vec<(K, V)>
where
    K: Eq,
{
    type Key = K;
    type Value = V;

    fn hash_patch_apply(
        &mut self,
        patch: HashPatch<Self::Key, Self::Value>,
    ) -> Option<Self::Value> {
        match patch {
            HashPatch::Insert(k, v) => {
                if let Some(i) = self.iter().position(|(k_here, _)| k_here == &k) {
                    let kv = self.get_mut(i).unwrap();
                    Some(std::mem::replace(&mut kv.1, v))
                } else {
                    self.push((k, v));
                    None
                }
            }
            HashPatch::Remove(k) => {
                if let Some(i) = self.iter().position(|(k_here, _)| k_here == &k) {
                    Some(self.remove(i).1)
                } else {
                    None
                }
            }
        }
    }
}

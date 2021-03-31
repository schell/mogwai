//! Updating lists of items.
use std::ops::{Bound, RangeBounds};

/// Variants used to patch the items in a list.
#[derive(Clone, Debug)]
pub enum Patch<T> {
    /// Replace the specified range in the list with the given `replace_with` items.
    /// Zero-indexed.
    Splice {
        /// The index to insert the item.
        range: (Bound<usize>, Bound<usize>),
        /// The items.
        replace_with: Vec<T>,
    },

    /// Push an item onto the back of the list of items.
    Push {
        /// The item to push on the back.
        value: T,
    },

    /// Remove the last item.
    Pop,
}

impl<T> Patch<T> {
    pub fn patch_map<F, X>(&self, f: F) -> Patch<X>
    where
        F: Fn(&T) -> X,
    {
        match self {
            Patch::Splice {
                range,
                replace_with,
            } => Patch::Splice {
                range: *range,
                replace_with: replace_with.iter().map(f).collect::<Vec<_>>(),
            },
            Patch::Push { value } => Patch::Push { value: f(value) },
            Patch::Pop => Patch::Pop,
        }
    }
}

/// Provides `apply_patch` to list types.
pub trait PatchApply {
    type Item;

    /// Apply the given patch, modifying the list and returning the removed items.
    fn patch_apply(&mut self, patch: Patch<Self::Item>) -> Vec<Self::Item>;

    /// Insert the given item into the list at the given index, pushing all other items to the right.
    fn patch_insert(&mut self, index: usize, item: Self::Item) {
        let ts = self.patch_splice(index..index, vec![item]);
        assert!(ts.is_empty());
    }

    /// Swap the item at the given index with the given item. Return the original item, if possible.
    fn patch_swap(&mut self, index: usize, item: Self::Item) -> Option<Self::Item> {
        let mut ts = self.patch_splice(index..=index, vec![item]);
        assert!(ts.len() <= 1, "unexpected number of removed items");
        match ts.len() {
            0 => None,
            1 => ts.pop(),
            _ => unreachable!(),
        }
    }

    /// Remove the item at the give index. Return the original item, if possible.
    fn patch_remove(&mut self, index: usize) -> Option<Self::Item> {
        let mut ts = self.patch_splice(index..=index, vec![]);
        assert!(ts.len() <= 1, "unexpected number of removed items");
        match ts.len() {
            0 => None,
            1 => ts.pop(),
            _ => unreachable!(),
        }
    }

    /// Pushes the item to the end of the list.
    fn patch_push(&mut self, item: Self::Item) {
        let ts = self.patch_apply(Patch::Push { value: item });
        assert!(ts.is_empty());
    }

    /// Removes the last item and returns it, if possible.
    fn patch_pop(&mut self) -> Option<Self::Item> {
        let mut ts = self.patch_apply(Patch::Pop);
        assert!(ts.len() <= 1);
        ts.pop()
    }

    /// Replace the specified range in the list with the given `replace_with` items.
    /// Returns any removed items.
    fn patch_splice<R: RangeBounds<usize>, I:IntoIterator<Item = Self::Item>>(&mut self, range:R, replace_with:I) -> Vec<Self::Item> {
        let range = (range.start_bound().cloned(), range.end_bound().cloned());
        let replace_with = replace_with.into_iter().collect::<Vec<_>>();
        self.patch_apply(Patch::Splice { range, replace_with })
    }
}

impl<T> PatchApply for Vec<T> {
    type Item = T;

    fn patch_apply(&mut self, patch: Patch<T>) -> Vec<T> {
        match patch {
            Patch::Splice {
                range,
                replace_with,
            } => self.splice(range, replace_with).collect::<Vec<T>>(),
            Patch::Push { value } => {
                self.push(value);
                vec![]
            }
            Patch::Pop => self.pop().map(|t| vec![t]).unwrap_or_else(|| vec![]),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn vec_patching() {
        let mut vs = vec![0, 1, 2, 3, 4, 5];

        vs.patch_insert(2, 666);
        assert_eq!(&vs, &[0, 1, 666, 2, 3, 4, 5]);

        vs.patch_swap(2, 0xC0FFEE);
        assert_eq!(&vs, &[0, 1, 0xC0FFEE, 2, 3, 4, 5]);

        vs.patch_remove(2);
        assert_eq!(&vs, &[0, 1, 2, 3, 4, 5]);

        let _ = vs.patch_splice(0.., vec![]);
        assert!(&vs.is_empty());

        vs.patch_push(0);
        vs.patch_push(1);
        assert_eq!(&vs, &[0, 1]);

        let n = vs.patch_pop().unwrap();
        assert_eq!(n, 1);
        assert_eq!(&vs, &[0]);
    }
}

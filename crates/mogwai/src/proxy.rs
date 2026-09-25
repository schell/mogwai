//! # Data update mechanism
//!
//! `Proxy` provides a mechanism similar to JavaScript's Proxy object,
//! allowing for data updates in a single location to mutate multiple locations
//! within the view.
//!
//! The [`rsx!`](crate::view::rsx) macro has special support for `Proxy` that
//! make mutating views easy.
//!
//! Proxies use interior mutability, like `web-sys` types and the rest of
//! mogwai's view layers, so updating a proxy only requires shared access.
//! Widgets that only update their state through proxies can implement
//! [`Step`](crate::step::Step) and be raced by their parents, instead of
//! implementing [`StepMut`](crate::step::StepMut) and forcing their parents to
//! step them one at a time.

use std::{borrow::Cow, marker::PhantomData};

use crate::{
    sync::Shared,
    view::{AppendArg, View, ViewChild, ViewParent},
};

/// A proxy type that connects a view to some data that updates the view.
///
/// The proxy holds its model in a [`Shared`] and the update function in an
/// interior cell, so all methods take `&self`. The model is only accessible
/// through the proxy itself: reads go through [`with`](Self::with) and every
/// mutation goes through [`set`](Self::set) or [`modify`](Self::modify), which
/// trigger updates.
///
/// ```rust
/// use mogwai::prelude::*;
///
/// let proxy = Proxy::<u32>::default();
/// let clicks = proxy.with(|n| *n);
/// assert_eq!(0, clicks);
/// ```
#[derive(Default)]
pub struct Proxy<T> {
    model: Shared<T>,
    #[expect(clippy::type_complexity, reason = "not that complex")]
    update: Shared<Option<Box<dyn FnMut(&T) + 'static>>>,
}

impl<T: PartialEq> Proxy<T> {
    /// Sets the value of the proxy.
    ///
    /// If the new value is different from the current value, this triggers the
    /// update function, if it exists.
    pub fn set(&self, t: T) {
        let changed = self.model.with_mut(|model| {
            if t != *model {
                *model = t;
                true
            } else {
                false
            }
        });
        if changed {
            self.fire();
        }
    }
}

impl<T> Proxy<T> {
    /// Creates a new `Proxy` with the given model.
    pub fn new(model: T) -> Self {
        Self {
            model: Shared::new(model),
            update: Shared::new(None),
        }
    }

    /// Run a function with access to the inner value.
    ///
    /// This is the only way to read the model; mutations go through
    /// [`set`](Self::set) or [`modify`](Self::modify) and trigger updates.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.model.with(f)
    }

    /// Sets a function to be called whenever the model is updated.
    ///
    /// This function is used within the [`rsx!`](crate::view::rsx) macro to
    /// mutate the views the proxy is associated with.
    pub fn on_update(&self, f: impl FnMut(&T) + 'static) {
        self.update.with_mut(|update| {
            *update = Some(Box::new(f));
        });
    }

    /// Modifies the inner value.
    ///
    /// Triggers the update function if it exists.
    pub fn modify(&self, f: impl FnOnce(&mut T)) {
        self.model.with_mut(f);
        self.fire();
    }

    /// Run the update function, if any, with the current value of the model.
    ///
    /// The update function is taken out of its slot while it runs so that
    /// updates triggered from within it cannot borrow the slot reentrantly
    /// (a `RefCell` re-borrow panics on wasm32 and `RwLock` re-entry deadlocks
    /// elsewhere). An update triggered from within an update is swallowed
    /// instead of recursing.
    fn fire(&self) {
        let update = self.update.with_mut(|slot| slot.take());
        if let Some(mut update) = update {
            self.model.with(|model| update(model));
            self.update.with_mut(|slot| *slot = Some(update));
        }
    }
}

/// An internal type used by the [`rsx!`](crate::view::rsx) macro to replace
/// nodes in response to proxy value changes.
///
/// You shouldn't have to use this type manually, but it is public in support of
/// `rsx!`.
pub struct ProxyChild<V: View> {
    _phantom: PhantomData<V>,
    nodes: Vec<V::Node>,
}

impl<V: View> Clone for ProxyChild<V> {
    fn clone(&self) -> Self {
        Self {
            _phantom: PhantomData,
            nodes: self.nodes.clone(),
        }
    }
}

impl<V: View> ViewChild<V> for ProxyChild<V> {
    fn as_append_arg(
        &self,
    ) -> crate::prelude::AppendArg<V, impl Iterator<Item = Cow<'_, <V as View>::Node>>> {
        AppendArg::new(self.nodes.iter().map(Cow::Borrowed))
    }
}

impl<V: View> ProxyChild<V> {
    pub fn new(child: impl ViewChild<V>) -> Self {
        let mut nodes: Vec<V::Node> = vec![];
        for child in child.as_append_arg() {
            nodes.push(child.as_ref().clone());
        }
        Self {
            _phantom: PhantomData,
            nodes,
        }
    }

    pub fn replace(&mut self, parent: &V::Element, child: impl ViewChild<V>) {
        let mut previous_nodes = std::mem::take(&mut self.nodes).into_iter().rev();
        let mut new_nodes = child
            .as_append_arg()
            .map(Cow::into_owned)
            .collect::<Vec<_>>()
            .into_iter()
            .rev();
        loop {
            match (previous_nodes.next(), new_nodes.next()) {
                (Some(prev), Some(new)) => {
                    // Easiest case, both exist so we simply replace them.
                    parent.replace_node(Cow::Borrowed(&new), Cow::Borrowed(&prev));
                    self.nodes.push(new);
                }
                (Some(prev), None) => {
                    // We've run out of new nodes, remove the rest of the old
                    // nodes
                    parent.remove_node(Cow::Borrowed(&prev));
                }
                (None, Some(new)) => {
                    // We've run out of old nodes, add the new one before the
                    // last new one.
                    //
                    // Here the "last" new one is actually the head of the list,
                    // since we're iterating over the
                    // reverse.
                    parent.insert_node_before(
                        Cow::Borrowed(&new),
                        self.nodes.last().map(Cow::Borrowed),
                    );
                    self.nodes.push(new);
                }
                (None, None) => {
                    self.nodes.reverse();
                    return;
                }
            }
        }
    }
}

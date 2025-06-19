//! # Data update mechanism
//!
//! `Proxy` provides a mechanism similar to JavaScript's Proxy object,
//! allowing for data updates in a single location to mutate multiple locations within
//! the view.
//!
//! The [`rsx!`](crate::view::rsx) macro has special support for `Proxy` that make
//! mutating views easy.

use std::{borrow::Cow, marker::PhantomData, ops::Deref};

use crate::view::{AppendArg, View, ViewChild, ViewParent};

/// A proxy type that connects a view to some data that updates the view.
#[derive(Default)]
pub struct Proxy<T> {
    model: T,
    #[expect(clippy::type_complexity, reason = "not that complex")]
    update: Option<Box<dyn FnMut(&T) + 'static>>,
}

impl<T> Deref for Proxy<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl<T> AsRef<T> for Proxy<T> {
    fn as_ref(&self) -> &T {
        &self.model
    }
}

impl<T: PartialEq> Proxy<T> {
    /// Sets the value of the proxy.
    ///
    /// If the new value is different from the current value,
    /// it the update function, if it exists.
    pub fn set(&mut self, t: T) {
        println!("setting proxy");
        if t != self.model {
            self.model = t;
            if let Some(update) = self.update.as_mut() {
                update(&self.model);
            }
        } else {
            println!("proxy is unchanged");
        }
    }
}

impl<T> Proxy<T> {
    /// Creates a new `Proxy` with the given model.
    pub fn new(model: T) -> Self {
        Self {
            model,
            update: None,
        }
    }

    /// Sets a function to be called whenever the model is updated.
    ///
    /// This function is used within the [`rsx!`](crate::view::rsx) macro to mutate the views the
    /// proxy is associated with.
    pub fn on_update(&mut self, f: impl FnMut(&T) + 'static) {
        self.update = Some(Box::new(f))
    }

    /// Modifies the inner value.
    ///
    /// Triggers the update function if it exists.
    pub fn modify(&mut self, f: impl FnOnce(&mut T)) {
        println!("modifying proxy");
        f(&mut self.model);
        if let Some(update) = self.update.as_mut() {
            update(&self.model);
        }
    }
}

/// An internal type used by the [`rsx!`](crate::view::rsx) macro to replace nodes in
/// response to proxy value changes.
///
/// You shouldn't have to use this type manually, but it is public in support of `rsx!`.
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
                    // We've run out of new nodes, remove the rest of the old nodes
                    parent.remove_node(Cow::Borrowed(&prev));
                }
                (None, Some(new)) => {
                    // We've run out of old nodes, add the new one before the last
                    // new one.
                    //
                    // Here the "last" new one is actually the head of the list, since
                    // we're iterating over the reverse.
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

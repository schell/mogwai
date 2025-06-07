//! Proxy for data updates.
//!
//! [`Proxy`] is a little like Javascript's Proxy object.
//!
//! When a [`Proxy`] is updated, it propogates to the views that it was
//! created with.

use std::{borrow::Cow, marker::PhantomData, ops::Deref};

use crate::view::{AppendArg, View, ViewChild, ViewParent};

/// A proxy type that connects a view to some data that updates the view.
pub struct Proxy<V: View, T> {
    model: T,
    #[expect(clippy::type_complexity, reason = "not that complex")]
    update: Option<Box<dyn FnMut(&T) + 'static>>,
    _phantom: PhantomData<V>,
}

impl<V: View, T: Default> Default for Proxy<V, T> {
    fn default() -> Self {
        Self {
            model: Default::default(),
            update: Default::default(),
            _phantom: Default::default(),
        }
    }
}

impl<V: View, T> Deref for Proxy<V, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl<V: View, T> AsRef<T> for Proxy<V, T> {
    fn as_ref(&self) -> &T {
        &self.model
    }
}

impl<V: View, T: PartialEq> Proxy<V, T> {
    pub fn set(&mut self, t: T) {
        if t != self.model {
            self.model = t;
            if let Some(update) = self.update.as_mut() {
                update(&self.model);
            }
        }
    }
}

impl<V: View, T> Proxy<V, T> {
    pub fn new(model: T) -> Self {
        Self {
            model,
            update: None,
            _phantom: PhantomData,
        }
    }

    pub fn on_update(&mut self, f: impl FnMut(&T) + 'static) {
        self.update = Some(Box::new(f))
    }

    pub fn modify(&mut self, f: impl FnOnce(&mut T)) {
        f(&mut self.model);
        if let Some(update) = self.update.as_mut() {
            update(&self.model);
        }
    }
}

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
    pub fn new(parent: &V::Element, child: impl ViewChild<V>) -> Self {
        let mut nodes: Vec<V::Node> = vec![];
        for child in child.as_append_arg() {
            nodes.push(child.as_ref().clone());
            parent.append_node(child);
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

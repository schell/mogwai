//! Proxy for data updates.
//!
//! [`Proxy`] is a little like Javascript's Proxy object.
//!
//! When a [`Proxy`] is updated, it propogates to the views that it was
//! created with.

use std::{marker::PhantomData, ops::Deref};

use crate::view::{View, ViewChild, ViewParent};

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

impl<V: View, T: PartialEq> Proxy<V, T> {
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

    pub fn set(&mut self, t: T) {
        if t != self.model {
            self.model = t;
            if let Some(update) = self.update.as_mut() {
                update(&self.model);
            }
        }
    }
}

pub struct ProxyChild<V: View> {
    _phantom: PhantomData<V>,
    nodes: Vec<V::Node<'static>>,
}

impl<V: View> ProxyChild<V> {
    pub fn new(parent: &V::Element, child: impl ViewChild<V>) -> Self {
        let mut nodes: Vec<V::Node<'static>> = vec![];
        for child in child.as_append_arg() {
            parent.append_node(child.clone());
            nodes.push(child);
        }
        Self {
            _phantom: PhantomData,
            nodes,
        }
    }

    pub fn replace(&mut self, parent: &V::Element, child: impl ViewChild<V>) {
        let mut previous_nodes = std::mem::take(&mut self.nodes).into_iter().rev();
        let mut new_nodes = child.as_append_arg().rev();
        let mut last_new_node = None;
        let mut push_new = |new_node| {
            last_new_node = Some(&new_node);
            self.nodes.push(new_node);
        };
        loop {
            match (previous_nodes.next(), new_nodes.next()) {
                (Some(prev), Some(new)) => {
                    parent.replace_node(&new, &prev);
                    push_new(new);
                }
                (Some(prev), None) => {
                    // We've run out of new nodes, remove the rest of the old nodes
                    parent.remove_node(&prev);
                }
                (None, Some(new)) => {
                    // We've run out of old nodes, add the new one before the last
                    // new one.
                    parent.insert_node_before(&new, last_new_node);
                    push_new(new);
                }
                (None, None) => {
                    self.nodes.reverse();
                    return;
                }
            }
        }
    }
}

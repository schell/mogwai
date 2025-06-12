//! # Cross-platform view traits
//!
//! This module defines traits for building and managing views across different platforms.
//! It provides a flexible interface for creating, updating, and interacting with UI components
//! in a platform-agnostic manner.
use std::{borrow::Cow, marker::PhantomData};

use crate::Str;

pub use mogwai_macros::{ViewChild, rsx};

/// Trait for managing text content within a view.
///
/// The `ViewText` trait provides methods for creating, setting, and retrieving
/// text content in a view-compatible format. It is designed to be implemented
/// by types that represent text nodes in a view, allowing for consistent
/// manipulation of text across different platforms.
pub trait ViewText {
    /// Creates a new instance of the text node with the specified content.
    fn new(text: impl AsRef<str>) -> Self;
    /// Updates the text content of the node.
    fn set_text(&self, text: impl AsRef<str>);
    /// Retrieves the current text content of the node.
    fn get_text(&self) -> Str;
}

/// Marker trait providing extension methods for converting
/// strings into view-compatible formats.
pub trait ViewTextExt {
    fn into_text<V: View>(self) -> V::Text;
}

impl<T: AsRef<str>> ViewTextExt for T {
    fn into_text<V: View>(self) -> V::Text {
        ViewText::new(self)
    }
}

/// An internal type used for managing child nodes within a view.
///
/// `AppendArg` abracts over an iterator of child nodes, allowing implementations
/// of [`ViewChild`] to be written for iterators and single values alike.
///
/// `AppendArg` is primarily for internal use within the framework, but it is
/// exposed to facilitate the implementation of view-related traits. It provides
/// a mechanism for iterating over nodes that can be appended to a view.
pub struct AppendArg<V: View, I> {
    pub iter: I,
    _phantom: PhantomData<V>,
}

impl<V: View, C: ViewChild<V>, T: Iterator<Item = C>> From<T> for AppendArg<V, T> {
    fn from(iter: T) -> Self {
        AppendArg {
            iter,
            _phantom: PhantomData,
        }
    }
}

impl<V: View, T> From<T> for AppendArg<V, Option<T>> {
    fn from(value: T) -> Self {
        AppendArg {
            iter: Some(value),
            _phantom: PhantomData,
        }
    }
}

impl<V: View, I> AppendArg<V, I> {
    pub fn new(iter: I) -> Self {
        AppendArg {
            iter,
            _phantom: PhantomData,
        }
    }
}

impl<V: View, I: Iterator> Iterator for AppendArg<V, I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Defines methods for managing child nodes within a view.
///
/// This trait provides methods for appending, removing, and replacing child
/// nodes, as well as managing their order within the view.
pub trait ViewParent<V: View> {
    fn append_node(&self, node: Cow<'_, V::Node>);
    fn remove_node(&self, node: Cow<'_, V::Node>);
    fn replace_node(&self, new_node: Cow<'_, V::Node>, old_node: Cow<'_, V::Node>);
    fn insert_node_before(&self, new_node: Cow<'_, V::Node>, before_node: Option<Cow<'_, V::Node>>);

    fn append_child(&self, child: impl ViewChild<V>) {
        for node in child.as_append_arg() {
            self.append_node(node);
        }
    }
    fn remove_child(&self, child: impl ViewChild<V>) {
        for node in child.as_append_arg() {
            self.remove_node(node);
        }
    }
    fn replace_child(&self, new_child: impl ViewChild<V>, old_child: impl ViewChild<V>) {
        let new_nodes = new_child.as_append_arg();
        let old_nodes = old_child.as_append_arg();
        for (new_node, old_node) in new_nodes.zip(old_nodes) {
            self.replace_node(new_node, old_node);
        }
    }
    fn insert_child_before(
        &self,
        child: impl ViewChild<V>,
        before_child: Option<impl ViewChild<V>>,
    ) {
        if let Some(before_child) = before_child {
            let mut before_nodes = before_child.as_append_arg();
            for new_node in child.as_append_arg() {
                self.insert_node_before(new_node, before_nodes.next());
            }
        } else {
            self.append_child(child);
        }
    }
}

/// Represents a node that can be appended to a view.
///
/// This trait provides a method for converting a node into an appendable
/// format, allowing it to be added to a view.
///
/// Deriving `ViewChild` for a Rust type allows it to be included in the
/// node position of an [`rsx!`] macro.
pub trait ViewChild<V: View> {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = Cow<'_, V::Node>>>;
}

impl<V: View, T: ViewChild<V>> ViewChild<V> for &T {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = Cow<'_, V::Node>>> {
        T::as_append_arg(self)
    }
}

impl<V: View, T: ViewChild<V> + 'static> ViewChild<V> for &mut T {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = Cow<'_, V::Node>>> {
        T::as_append_arg(self)
    }
}

impl<V: View, T: ViewChild<V>> ViewChild<V> for Vec<T> {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = Cow<'_, V::Node>>> {
        AppendArg::new(self.iter().flat_map(|t| t.as_append_arg()))
    }
}

impl<V: View, T: ViewChild<V>> ViewChild<V> for Option<T> {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = Cow<'_, V::Node>>> {
        AppendArg::new(self.iter().flat_map(|t| t.as_append_arg()))
    }
}

impl<V: View> ViewChild<V> for String {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = Cow<'_, V::Node>>> {
        let text = self.into_text::<V>();
        let mut arg = text.as_append_arg();
        // UNWRAP: safe because we created the text.
        let node: V::Node = arg.next().unwrap().into_owned();
        AppendArg::new(std::iter::once(Cow::Owned(node)))
    }
}

/// Manages properties and styles of view elements.
///
/// This trait provides methods for setting, getting, and removing properties
/// and styles from view elements.
pub trait ViewProperties {
    /// Returns whether this view has a property with the given name set.
    fn has_property(&self, property: impl AsRef<str>) -> bool;
    /// Get the value of the given property, if any.
    fn get_property(&self, property: impl AsRef<str>) -> Option<Str>;
    /// Sets the property on the view.
    fn set_property(&self, property: impl AsRef<str>, value: impl AsRef<str>);
    /// Remove an attribute.
    fn remove_property(&self, property: impl AsRef<str>);

    /// Add a style property.
    fn set_style(&self, key: impl AsRef<str>, value: impl AsRef<str>);
    /// Remove a style property.
    ///
    /// Returns the previous style value, if any.
    fn remove_style(&self, key: impl AsRef<str>);
}

/// Handles event listening for view elements.
///
/// This trait provides methods for attaching event listeners to global
/// things (window and document) and handling events asynchronously.
pub trait ViewEventListener<V: View> {
    /// Returns a future that resolves on the next event occurence.
    fn next(&self) -> impl Future<Output = V::Event>;
    fn on_window(event_name: impl Into<Cow<'static, str>>) -> V::EventListener;
    fn on_document(event_name: impl Into<Cow<'static, str>>) -> V::EventListener;
}

/// Defines methods for attaching event listeners to view elements.
///
/// This trait provides a method for listening to events on a view element,
/// enabling interaction with user actions.
pub trait ViewEventTarget<V: View> {
    fn listen(&self, event_name: impl Into<Cow<'static, str>>) -> V::EventListener;
}

/// Defines methods for creating and using elements.
///
/// Represents an element within a view, providing platform-specific operations.
pub trait ViewElement {
    type View: View<Element = Self>;

    fn new(name: impl AsRef<str>) -> Self;

    fn new_namespace(name: impl AsRef<str>, ns: impl AsRef<str>) -> Self
    where
        Self: ViewProperties + Sized,
    {
        let el = Self::new(name);
        el.set_property("xmlns", ns);
        el
    }

    /// Attempt to perform a platform-specific operation on the given element.
    fn when_element<V: View, T>(&self, f: impl FnOnce(&V::Element) -> T) -> Option<T> {
        let el = try_cast_el::<Self::View, V>(self)?;
        let t = f(el);
        Some(t)
    }
}

/// Represents an event within a view, providing platform-specific operations.
pub trait ViewEvent {
    type View: View<Event = Self>;

    /// Attempt to perform a platform-specific operation with the given event.
    fn when_event<V: View, T>(&self, f: impl FnOnce(&V::Event) -> T) -> Option<T> {
        let el = try_cast_ev::<Self::View, V>(self)?;
        let t = f(el);
        Some(t)
    }
}

/// The core trait that defines the structure and behavior of a view.
///
/// This trait outlines the essential components of a view, including nodes,
/// elements, text, event listeners, and events, providing a comprehensive
/// interface for building and managing views.
pub trait View: Sized + 'static {
    type Node: Clone;
    type Element: ViewElement
        + ViewParent<Self>
        + ViewChild<Self>
        + ViewProperties
        + ViewEventTarget<Self>
        + Clone
        + 'static;
    type Text: ViewText + ViewChild<Self> + ViewEventTarget<Self> + Clone + 'static;
    type EventListener: ViewEventListener<Self>;
    type Event: ViewEvent;
}

fn try_cast_el<V: View, W: View>(element: &V::Element) -> Option<&W::Element> {
    // Pay no attention to the man behind the curtain.
    if std::any::TypeId::of::<W>() == std::any::TypeId::of::<V>() {
        // Nothing to see here!
        Some(unsafe { &*(element as *const V::Element as *const W::Element) })
    } else {
        None
    }
}

fn try_cast_ev<V: View, W: View>(event: &V::Event) -> Option<&W::Event> {
    if std::any::TypeId::of::<W>() == std::any::TypeId::of::<V>() {
        // Nothing to see here!
        Some(unsafe { &*(event as *const V::Event as *const W::Event) })
    } else {
        None
    }
}

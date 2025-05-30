//! Traits for building cross-platform views.
use std::{marker::PhantomData, ops::Deref};

use crate::Str;

pub use mogwai_future_rsx::{FromBuilder, ViewChild};

pub trait ViewText {
    fn new(text: impl Into<Str>) -> Self;
    fn set_text(&self, text: impl Into<Str>);
    fn get_text(&self) -> Str;
}

pub trait ViewTextExt {
    fn into_text<V: View>(self) -> V::Text;
}

impl ViewTextExt for String {
    fn into_text<V: View>(self) -> V::Text {
        ViewText::new(Str::from(self))
    }
}

impl ViewTextExt for &String {
    fn into_text<V: View>(self) -> V::Text {
        ViewText::new(Str::from(self))
    }
}

impl ViewTextExt for &str {
    fn into_text<V: View>(self) -> V::Text {
        ViewText::new(Str::from(self.to_owned()))
    }
}

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

pub trait ViewParent<V: View> {
    fn new(name: impl Into<Str>) -> Self;
    fn new_namespace(name: impl Into<Str>, ns: impl Into<Str>) -> Self;
    fn append_child(&self, child: impl ViewChild<V>);
    fn remove_child(&self, child: impl ViewChild<V>);
}

pub trait ViewChild<V: View> {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = V::Node>>;
}

impl<V: View, T: ViewChild<V> + 'static> ViewChild<V> for &T {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = V::Node>> {
        (*self).as_append_arg()
    }
}

impl<V: View, T: ViewChild<V>> ViewChild<V> for Vec<T> {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = V::Node>> {
        AppendArg::new(self.iter().flat_map(|t| t.as_append_arg()))
    }
}

pub trait ViewProperties {
    /// Returns whether this view has a property with the given name set.
    fn has_property(&self, property: impl AsRef<str>) -> bool;
    /// Get the value of the given property, if any.
    fn get_property(&self, property: impl AsRef<str>) -> Option<Str>;
    /// Sets the property on the view.
    fn set_property(&self, property: impl Into<Str>, value: impl Into<Str>);
    /// Remove an attribute.
    fn remove_property(&self, property: impl AsRef<str>);

    /// Add a style property.
    fn set_style(&self, key: impl Into<Str>, value: impl Into<Str>);
    /// Remove a style property.
    ///
    /// Returns the previous style value, if any.
    fn remove_style(&self, key: impl AsRef<str>);
}

pub trait ViewEventListener<V: View> {
    type Event;

    fn next(&self) -> impl Future<Output = Self::Event>;
}

pub trait ViewEventTarget<V: View> {
    fn listen(&self, event_name: impl Into<Str>) -> V::EventListener;
}

pub trait ViewCast<V: View> {
    fn cast_element<T>(element: V::Element) -> V::El<T>;
}

// TODO: split this into types and ops
pub trait View: Sized + 'static {
    type Element: ViewParent<Self>
        + ViewChild<Self>
        + ViewProperties
        + ViewEventTarget<Self>
        + 'static;
    type Text: ViewText + ViewChild<Self> + ViewEventTarget<Self> + Clone + 'static;
    type Node;
    type EventListener: ViewEventListener<Self>;
    type El<T: ViewCast<Self>>;
}

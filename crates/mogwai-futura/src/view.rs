//! Traits for building cross-platform views.
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

pub struct AppendArg<I> {
    pub iter: I,
}

impl<C: ViewChild, T: Iterator<Item = C>> From<T> for AppendArg<T> {
    fn from(iter: T) -> Self {
        AppendArg { iter }
    }
}

impl<T> From<T> for AppendArg<Option<T>> {
    fn from(value: T) -> Self {
        AppendArg { iter: Some(value) }
    }
}

impl<I> AppendArg<I> {
    pub fn new(iter: I) -> Self {
        AppendArg { iter }
    }
}

impl<I: Iterator> Iterator for AppendArg<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub trait ViewParent {
    type Node;

    fn append_child(&self, child: impl ViewChild<Node = Self::Node>);
    fn remove_child(&self, child: impl ViewChild<Node = Self::Node>);
}

pub trait ViewChild {
    type Node;

    fn as_append_arg(&self) -> AppendArg<impl Iterator<Item = Self::Node>>;
}

impl<T: ViewChild + 'static> ViewChild for &T {
    type Node = T::Node;

    fn as_append_arg(&self) -> AppendArg<impl Iterator<Item = Self::Node>> {
        (*self).as_append_arg()
    }
}

impl<T: ViewChild> ViewChild for Vec<T> {
    type Node = T::Node;

    fn as_append_arg(&self) -> AppendArg<impl Iterator<Item = Self::Node>> {
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
}

pub trait ViewEventListener {
    type Event;

    fn next(&self) -> impl Future<Output = Self::Event>;
}

pub trait ViewEventTarget {
    fn listen(&self, event_name: impl Into<Str>) -> impl ViewEventListener;
}

pub trait View {
    type Element<T>: ViewParent + ViewChild + ViewProperties
    where
        T: ViewParent + ViewChild + ViewProperties;
    type Text: ViewText + ViewChild + Clone;
    type EventListener: ViewEventListener;
}

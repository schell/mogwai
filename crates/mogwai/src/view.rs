//! Traits for building cross-platform views.
use std::{borrow::Cow, marker::PhantomData};

use crate::Str;

pub use mogwai_macros::{ViewChild, rsx};

pub trait ViewText {
    fn new(text: impl AsRef<str>) -> Self;
    fn set_text(&self, text: impl AsRef<str>);
    fn get_text(&self) -> Str;
}

pub trait ViewTextExt {
    fn into_text<V: View>(self) -> V::Text;
}

impl<T: AsRef<str>> ViewTextExt for T {
    fn into_text<V: View>(self) -> V::Text {
        ViewText::new(self)
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
    fn new(name: impl AsRef<str>) -> Self;
    fn new_namespace(name: impl AsRef<str>, ns: impl AsRef<str>) -> Self;

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
}

pub trait ViewChild<V: View> {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = Cow<'_, V::Node>>>;
}

impl<V: View, T: ViewChild<V> + 'static> ViewChild<V> for &T {
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

pub trait ViewEventListener<V: View> {
    fn next(&self) -> impl Future<Output = V::Event>;
    fn on_window(event_name: impl Into<Cow<'static, str>>) -> V::EventListener;
    fn on_document(event_name: impl Into<Cow<'static, str>>) -> V::EventListener;
}

pub trait ViewEventTarget<V: View> {
    fn listen(&self, event_name: impl Into<Cow<'static, str>>) -> V::EventListener;
}

pub trait ViewElement {
    type View: View<Element = Self>;

    fn when_element<V: View, T>(&self, f: impl FnOnce(&V::Element) -> T) -> Option<T> {
        let el = try_cast_el::<Self::View, V>(self)?;
        let t = f(el);
        Some(t)
    }
}

pub trait ViewEvent {
    type View: View<Event = Self>;

    fn when_event<V: View, T>(&self, f: impl FnOnce(&V::Event) -> T) -> Option<T> {
        let el = try_cast_ev::<Self::View, V>(self)?;
        let t = f(el);
        Some(t)
    }
}

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

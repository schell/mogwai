//! Traits for constructing and modifying declarative views.
//!
//! These traits help define behavior that can be implemented
//! with web_sys DOM nodes as well as server-side nodes.
use async_channel::Sender;
use futures::Stream;

pub mod builder;

/// `ElementView`s are views that represent DOM elements.
pub trait ElementView {
    /// Create a view with the given element tag.
    fn element_view(tag: &str) -> Self;

    /// Create a view with the given element tag and namespace.
    fn element_ns_view(tag: &str, ns: &str) -> Self;
}

/// `TextView`s are views that represent text nodes.
pub trait TextView<'a> {
    /// Create a new text node view that gets its inner text from the
    /// given stream.
    fn text_view<S, T>(&mut self, t: T)
    where
        String: From<S>,
        S: 'a,
        T: Stream<Item = S> + 'a;
}

/// `AttributeView`s can describe themselves with key value pairs.
pub trait AttributeView<'a> {
    /// Create a named attribute on the view that may change with each value of
    /// the given stream.
    fn attribute<T>(&mut self, t: T)
    where
        T: Stream<Item = crate::patch::HashPatch<String, String>> + 'a;

    /// Create (or remove) a boolean attribute on the view that may change its
    /// value with each new value of the given stream.
    fn boolean_attribute<T>(&mut self, t: T)
    where
        T: Stream<Item = crate::patch::HashPatch<String, bool>> + 'a;
}

/// `StyleView`s can describe themselves using CSS style key value pairs.
pub trait StyleView<'a> {
    /// Set a CSS property in the style attribute of the view being built.
    /// If `eff` contains a Receiver, messages received will updated the style's
    /// value.
    fn style<T>(&mut self, t: T)
    where
        T: Stream<Item = crate::patch::HashPatch<String, String>> + 'a;
}

/// `EventTargetView`s can send messages when events occur within them. They can
/// also transmit messages when events occur within the window or the document.
pub trait EventTargetView<Event> {
    /// Transmit an event message on the given sender when the named event
    /// happens.
    fn on(&mut self, ev_name: &str, tx: Sender<Event>);

    /// Transmit an event message on the given sender when the named event
    /// happens on the "window".
    fn window_on(&mut self, ev_name: &str, tx: Sender<Event>);

    /// Transmit an event message into the given transmitter when the named event
    /// happens on the "document".
    fn document_on(&mut self, ev_name: &str, tx: Sender<Event>);
}

/// `PatchView`s can have their child nodes updated with `ListPatch` messages.
pub trait PatchView<'a, Child> {
    /// Patch the view using a [`ListPatch`] message stream.
    fn patch<T>(&mut self, t: T)
    where
        T: Stream<Item = crate::patch::ListPatch<Child>> + 'a;
}

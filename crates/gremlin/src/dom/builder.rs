//! A low cost intermediate structure for creating views.
//!
//! Here we attempt to have our cake and eat it too.
use futures::StreamExt;
use std::pin::Pin;

use crate::{
    dom::*,
    patch::{HashPatch, ListPatch},
};

/// Attribute declaration streams.
pub enum AttributeCmd<'a> {
    /// HashPatch updates for String attributes.
    Attrib(Pin<Box<dyn Stream<Item = HashPatch<String, String>> + 'a>>),
    /// HashPatch updates for boolean attributes.
    Bool(Pin<Box<dyn Stream<Item = HashPatch<String, bool>> + 'a>>),
}

/// A single style declaration.
pub struct StyleCmd<'a>(pub Pin<Box<dyn Stream<Item = HashPatch<String, String>> + 'a>>);

/// An event target declaration.
#[derive(Clone)]
pub enum EventTargetType {
    /// This target is the view it is declared on.
    Myself,
    /// This target is the window.
    Window,
    /// This target is the document.
    Document,
}

/// A DOM event declaration.
#[derive(Clone)]
pub struct EventTargetCmd<Event> {
    /// The target of the event.
    /// In other words this is the target that a listener will be placed on.
    pub type_is: EventTargetType,
    /// The event name.
    pub name: String,
    /// The [`Sender`] that the event should be sent on.
    pub transmitter: Sender<Event>,
}

/// Child patching declaration.
pub struct PatchCmd<'a, T>(pub Pin<Box<dyn Stream<Item = ListPatch<T>> + 'a>>);

/// An un-built mogwai view.
/// A ViewBuilder is the most generic view representation in the mogwai library.
/// It is the the blueprint of a view - everything needed to create, hydrate or serialize the view.
pub struct ViewBuilder<'a, T, Event> {
    /// Construction argument string.
    pub construct_with: Option<String>,
    /// Optional namespace.
    pub ns: Option<String>,
    /// This element's text if it is a text node.
    pub text: Option<Pin<Box<dyn Stream<Item = String> + 'a>>>,
    /// This view's attribute declarations.
    pub attribs: Vec<AttributeCmd<'a>>,
    /// This view's style declarations.
    pub styles: Vec<StyleCmd<'a>>,
    /// This view's output events.
    pub events: Vec<EventTargetCmd<Event>>,
    /// This view's child patch receivers.
    pub patches: Vec<PatchCmd<'a, T>>,
}

impl<'a, T, Event> Default for ViewBuilder<'a, T, Event> {
    fn default() -> Self {
        ViewBuilder {
            construct_with: None,
            ns: None,
            text: None,
            attribs: vec![],
            styles: vec![],
            events: vec![],
            patches: vec![],
        }
    }
}

/// # ElementView

impl<'a, T, Event> ElementView for ViewBuilder<'a, T, Event> {
    fn element_view(tag: &str) -> Self {
        let mut builder = ViewBuilder::default();
        builder.construct_with = Some(tag.into());
        builder
    }

    fn element_ns_view(tag: &str, ns: &str) -> Self {
        let mut builder = ViewBuilder::default();
        builder.construct_with = Some(tag.into());
        builder.ns = Some(ns.into());
        builder
    }
}

/// # TextView

impl<'a, V, Event> TextView<'a> for ViewBuilder<'a, V, Event> {
    fn text_view<S, T>(&mut self, t: T)
    where
        String: From<S>,
        S: 'a,
        T: Stream<Item = S> + 'a,
    {
        self.text = Some(t.map(String::from).boxed_local());
    }
}

/// # AttributeView

impl<'a, V, Event> AttributeView<'a> for ViewBuilder<'a, V, Event> {
    fn attribute<T>(&mut self, t: T)
    where
        T: Stream<Item = crate::patch::HashPatch<String, String>> + 'a,
    {
        self.attribs.push(AttributeCmd::Attrib(t.boxed_local()));
    }

    fn boolean_attribute<T>(&mut self, t: T)
    where
        T: Stream<Item = crate::patch::HashPatch<String, bool>> + 'a,
    {
        self.attribs.push(AttributeCmd::Bool(t.boxed_local()));
    }
}

/// # StyleView

impl<'a, V, Event> StyleView<'a> for ViewBuilder<'a, V, Event> {
    fn style<T>(&mut self, t: T)
    where
        T: Stream<Item = crate::patch::HashPatch<String, String>> + 'a,
    {
        self.style(t.boxed_local());
    }
}

/// # EventTargetView

impl<'a, T, Event> EventTargetView<Event> for ViewBuilder<'a, T, Event> {
    fn on(&mut self, ev_name: &str, tx: Sender<Event>) {
        self.events.push(EventTargetCmd {
            type_is: EventTargetType::Myself,
            name: ev_name.to_string(),
            transmitter: tx,
        });
    }

    fn window_on(&mut self, ev_name: &str, tx: Sender<Event>) {
        self.events.push(EventTargetCmd {
            type_is: EventTargetType::Window,
            name: ev_name.to_string(),
            transmitter: tx,
        });
    }

    fn document_on(&mut self, ev_name: &str, tx: Sender<Event>) {
        self.events.push(EventTargetCmd {
            type_is: EventTargetType::Document,
            name: ev_name.to_string(),
            transmitter: tx,
        });
    }
}

/// # PatchView

impl<'a, V, Event> PatchView<'a, V> for ViewBuilder<'a, V, Event> {
    fn patch<T>(&mut self, t: T)
    where
        T: Stream<Item = crate::patch::ListPatch<V>> + 'a,
    {
        self.patches.push(PatchCmd(t.boxed_local()));
    }
}

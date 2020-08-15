pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlInputElement};

use super::super::txrx::{Receiver, Transmitter};
pub use super::utils;


/// `Effect`s describe the state of something right now and what it will be in the
/// future after receiving a message through a `Receiver`.
pub enum Effect<T> {
    OnceNow { now: T },
    ManyLater { later: Receiver<T> },
    OnceNowAndManyLater { now: T, later: Receiver<T> },
}


impl<T> Effect<T> {
    pub fn into_some(self) -> (Option<T>, Option<Receiver<T>>) {
        match self {
            Effect::OnceNow { now } => (Some(now), None),
            Effect::ManyLater { later } => (None, Some(later)),
            Effect::OnceNowAndManyLater { now, later } => (Some(now), Some(later)),
        }
    }
}


impl<T> From<T> for Effect<T> {
    fn from(now: T) -> Effect<T> {
        Effect::OnceNow { now }
    }
}


impl From<&str> for Effect<String> {
    fn from(s: &str) -> Effect<String> {
        Effect::OnceNow { now: s.into() }
    }
}


impl<T> From<Receiver<T>> for Effect<T> {
    fn from(later: Receiver<T>) -> Effect<T> {
        Effect::ManyLater { later }
    }
}


impl<T> From<(T, Receiver<T>)> for Effect<T> {
    fn from((now, later): (T, Receiver<T>)) -> Effect<T> {
        Effect::OnceNowAndManyLater { now, later }
    }
}


impl From<(&str, Receiver<String>)> for Effect<String> {
    fn from((now, later): (&str, Receiver<String>)) -> Effect<String> {
        Effect::OnceNowAndManyLater {
            now: now.into(),
            later,
        }
    }
}


/// `ElementView`s are views that represent DOM elements.
pub trait ElementView {
    /// Create a view with the given element tag.
    fn element(tag: &str) -> Self;

    /// Create a view with the given element tag and namespace.
    fn element_ns(tag: &str, ns: &str) -> Self;

    /// Create a view from an existing element with the given id.
    /// Returns None if it cannot be found.
    // TODO: Determine if this is necessary
    fn from_element_by_id(id: &str) -> Option<Self>
    where
        Self: Sized;
}


/// `AttributeView`s can describe themselves with key value pairs.
pub trait AttributeView {
    /// Create a named attribute on the view that may change over time as
    /// a receiver receives a message.
    /// Here's an example that builds a gizmo with inital id and class values,
    /// and updates the class value whenever a message is received on a Receiver:
    ///
    /// ```rust, no_run
    /// extern crate mogwai;
    /// use mogwai::prelude::*;
    ///
    /// let (tx, rx) = txrx::<String>();
    /// let my_div = (View::element("div") as View<HtmlElement>)
    ///     .attribute("id", "my_div")
    ///     .attribute("class", ("hero_div", rx.branch_map(|class_update| {
    ///         ["hero_div", class_update].join(" ")
    ///     })));
    /// ```
    ///
    /// Alternatively you can use macros to define an equivalent view in RSX:
    ///
    /// ```rust, no_run
    /// extern crate mogwai;
    /// use mogwai::prelude::*;
    ///
    /// let (tx, rx) = txrx::<String>();
    /// let my_div:View<HtmlElement> = dom! {
    ///     <div id="my_div" class=("hero_div", rx.branch_map(|class_update| {
    ///         ["hero_div", class_update].join(" ")
    ///     })) />
    /// };
    /// ```
    fn attribute<E: Into<Effect<String>>>(self, name: &str, eff: E) -> Self;

    /// Create (or remove) a boolean attribute on the view that may change its
    /// value every time the given receiver receives a message
    /// If `eff` is a receiver and that receiver receives `false` it will
    /// respond by removing the attribute until it receives `true`. If `eff` is
    /// a single boolean value, either add or remove the attribute.
    fn boolean_attribute<E: Into<Effect<bool>>>(self, name: &str, eff: E) -> Self;
}


/// `StyleView`s can describe themselves using CSS style key value pairs.
pub trait StyleView {
    /// Set a CSS property in the style attribute of the view being built.
    /// If `eff` is a Receiver, this updates the style's value every time a
    /// message is received on the given `Receiver`.
    fn style<E: Into<Effect<String>>>(self, name: &str, eff: E) -> Self;
}


/// `EventTargetView`s can transmit messages when events occur within them. They can
/// also transmit messages when events occur within the window or the document.
pub trait EventTargetView {
    /// Transmit an event message on the given transmitter when the named event
    /// happens.
    fn on(self, ev_name: &str, tx: Transmitter<Event>) -> Self;

    /// Transmit an event message on the given transmitter when the named event
    /// happens on [`Window`].
    fn window_on(self, ev_name: &str, tx: Transmitter<Event>) -> Self;

    /// Transmit an event message into the given transmitter when the named event
    /// happens on [`Document`].
    fn document_on(self, ev_name: &str, tx: Transmitter<Event>) -> Self;
}


/// `ParentView`s can nest child views.
pub trait ParentView<T> {
    /// Add a child view to this parent.
    fn with(self, child: T) -> Self;
}


/// `PostBuildView`s can send their underlying browser DOM node as a message on
/// a Transmitter once they've been built.
///
/// This allows you to construct component behaviors that operate on the constructed
/// node directly, while still keeping the definition in its place within your view
/// builder function. For example, you may want to use `input.focus()` within the
/// `update` function of your component. This method allows you to store the
/// input `HtmlInputElement` once it is built.
pub trait PostBuildView {
    type DomNode;

    /// After the view is built, transmit its underlying DomNode on the given
    /// transmitter.
    fn post_build(self, tx: Transmitter<Self::DomNode>) -> Self;
}

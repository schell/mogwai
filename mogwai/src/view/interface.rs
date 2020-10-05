//! Interfaces for constructing declarative views.
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};

use crate::prelude::{Effect, Receiver, Transmitter};


/// `ElementView`s are views that represent DOM elements.
pub trait ElementView {
    /// Create a view with the given element tag.
    fn element(tag: &str) -> Self;

    /// Create a view with the given element tag and namespace.
    fn element_ns(tag: &str, ns: &str) -> Self;
}


/// `TextView`s are views that represent text nodes.
pub trait TextView {
    /// Create a new text node view.
    fn text<E: Into<Effect<String>>>(eff: E) -> Self;
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
    /// let mut my_div: View<HtmlElement> = View::element("div");
    /// my_div.attribute("id", "my_div");
    /// my_div.attribute("class", ("hero_div", rx.branch_map(|class_update| {
    ///     ["hero_div", class_update].join(" ")
    /// })));
    /// ```
    ///
    /// Alternatively you can use macros to define an equivalent view in RSX:
    ///
    /// ```rust, no_run
    /// extern crate mogwai;
    /// use mogwai::prelude::*;
    ///
    /// let (tx, rx) = txrx::<String>();
    /// let my_div:View<HtmlElement> = view! {
    ///     <div id="my_div" class=("hero_div", rx.branch_map(|class_update| {
    ///         ["hero_div", class_update].join(" ")
    ///     })) />
    /// };
    /// ```
    fn attribute<E: Into<Effect<String>>>(&mut self, name: &str, eff: E);

    /// Create (or remove) a boolean attribute on the view that may change its
    /// value every time the given receiver receives a message
    /// If `eff` contains a receiver and that receiver receives `false` it will
    /// respond by removing the attribute until it receives `true`. If `eff` is
    /// a single boolean value, either add or remove the attribute.
    fn boolean_attribute<E: Into<Effect<bool>>>(&mut self, name: &str, eff: E);
}


/// `StyleView`s can describe themselves using CSS style key value pairs.
pub trait StyleView {
    /// Set a CSS property in the style attribute of the view being built.
    /// If `eff` contains a Receiver, messages received will updated the style's
    /// value.
    fn style<E: Into<Effect<String>>>(&mut self, name: &str, eff: E);
}


/// `EventTargetView`s can transmit messages when events occur within them. They can
/// also transmit messages when events occur within the window or the document.
pub trait EventTargetView {
    /// Transmit an event message on the given transmitter when the named event
    /// happens.
    fn on(&mut self, ev_name: &str, tx: Transmitter<Event>);

    /// Transmit an event message on the given transmitter when the named event
    /// happens on [`Window`].
    fn window_on(&mut self, ev_name: &str, tx: Transmitter<Event>);

    /// Transmit an event message into the given transmitter when the named event
    /// happens on [`Document`].
    fn document_on(&mut self, ev_name: &str, tx: Transmitter<Event>);
}


/// `PostBuildView`s can send their underlying browser DOM node as a message on
/// a Transmitter once they've been built.
///
/// `PostBuildView` allows you to construct component behaviors that operate on the constructed
/// node directly, while still keeping the definition in its place within your view
/// builder function. For example, you may want to use `input.focus()` within the
/// `update` function of your component. This method allows you to store the
/// input `HtmlInputElement` once it is built, allowing you to use it as you see fit
/// within your [`Component::update`] function.
pub trait PostBuildView {
    type DomNode;

    /// After the view is built, transmit its underlying DomNode on the given
    /// transmitter.
    fn post_build(&mut self, tx: Transmitter<Self::DomNode>);
}


/// `ParentView`s can nest child views.
pub trait ParentView<T> {
    /// Add a child to this parent.
    fn with(&mut self, view_now: T);
}


/// `ReplaceView`s can entirely replace themselves with views sent to a
/// [`Receiver`].
pub trait ReplaceView<T> {
    fn this_later<S:Clone + Into<T> + 'static>(&mut self, rx: Receiver<S>);
}


/// An enumeration of commands used to update the children of a [`PatchView`].
#[derive(Clone, Debug)]
pub enum Patch<T> {
    Insert {
        index: usize,
        value: T
    },
    Replace {
        index: usize,
        value: T
    },
    Remove {
        index: usize
    },
    RemoveAll,
    PushFront {
        value: T
    },
    PushBack {
        value: T
    },
    PopFront,
    PopBack
}


impl<T> Patch<T> {
    pub(crate) fn branch_map<F, X>(&self, f:F) -> Patch<X>
    where
        F: FnOnce(&T) -> X
    {
        match self {
            Patch::Insert { index, value } => Patch::Insert {
                index: *index,
                value: f(value),
            },
            Patch::Replace { index, value } => Patch::Replace {
                index: *index,
                value: f(value),
            },
            Patch::Remove { index } => Patch::Remove { index: *index },
            Patch::RemoveAll => Patch::RemoveAll,
            Patch::PushFront { value } => Patch::PushFront {
                value: f(value),
            },
            Patch::PushBack { value } => Patch::PushBack {
                value: f(value),
            },
            Patch::PopFront => Patch::PopFront,
            Patch::PopBack => Patch::PopBack,
        }
    }
}


/// `PatchView`s' children can be manipulated using patch commands sent on a [`Receiver`].
pub trait PatchView<T> {
    fn patch<S:Clone + Into<T> + 'static>(&mut self, rx: Receiver<Patch<S>>);
}

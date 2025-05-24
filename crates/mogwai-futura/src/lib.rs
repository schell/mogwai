//! Future of mogwai.
//!
//! ## Impetus
//!
//! What I want is the ability to define a UI element and then render it
//! with various different platforms and have it behave similarly.
//!
//! An example of this would be creating a button that shows the number of
//! times it has been clicked, and then deploying that on the web, as a server-side
//! rendered string (after appying some number of artificial clicks) and also deploying
//! it in a terminal as a TUI.
//!
//! We might accomplish this with bare-bones Rust by defining the element in terms of a
//! model and a view interface. The model encodes the local state of the element
//! and its runtime logic, while the view interface determines how the runtime
//! logic can affect the view.
//!
//! The model is some concrete type, like `struct ButtonClicks {..}` and the view interface
//! would be a trait, `pub trait ButtonClicksInterface {..}`.
//!
//! Then each view platform ("web", "tui" and "ssr" in our case) could implement the view
//! interface and define the entry point.
//!
//! Model+logic and view.
//!
//! ### Model
//! Model is some concrete data that is used to update the view.
//! The type of the model cannot change from platform to platform.
//!
//! ### View Interface
//! A trait for interacting with the view in a cross-platform way.
//!
//! ### Logic
//! The logic is the computation that takes changes from the view through the interface,
//! updates the model and applies changes back through the interface.
//!
//! ### View
//! The view itself is responsible for rendering and providing events to the logic.
//! The type of the view changes depending on the platform.
//!
//! ## Strategy
//!
//! Mogwai's strategy towards solving the problem of cross-platform UI is not to offer
//! a one-size fits all view solution. Instead, `mogwai` aims to aid a _disciplined_
//! developer in modelling the UI using traits, and then providing the developer with
//! tools and wrappers to make fullfilling those traits on specific platforms as easy
//! as possible.

use std::{borrow::Cow, marker::PhantomData, ops::DerefMut};

use sync::Shared;

use web::event::EventListener;
#[cfg(feature = "web")]
use web_sys::wasm_bindgen::JsCast;

pub mod macros;
#[cfg(feature = "ssr")]
pub mod ssr;
pub mod sync;
pub mod tuple;
#[cfg(feature = "web")]
pub mod web;

/// A transparent wrapper around [`Cow<'static, str>`].
#[repr(transparent)]
#[derive(Clone, Default)]
pub struct Str {
    inner: Cow<'static, str>,
}

impl core::fmt::Display for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}

impl From<&'static str> for Str {
    fn from(s: &'static str) -> Self {
        Str { inner: s.into() }
    }
}

impl From<String> for Str {
    fn from(s: String) -> Self {
        Str { inner: s.into() }
    }
}

impl<'a> From<&'a String> for Str {
    fn from(s: &'a String) -> Self {
        Str {
            inner: s.clone().into(),
        }
    }
}

impl From<Cow<'static, str>> for Str {
    fn from(inner: Cow<'static, str>) -> Self {
        Str { inner }
    }
}

impl<'a> From<&'a Cow<'static, str>> for Str {
    fn from(s: &'a Cow<'static, str>) -> Self {
        Str { inner: s.clone() }
    }
}

impl Str {
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

pub trait ViewText {
    fn new(text: impl Into<Str>) -> Self;
    fn set_text(&self, text: impl Into<Str>);
}

impl ViewText for Shared<Str> {
    fn new(text: impl Into<Str>) -> Self {
        Shared::from(text.into())
    }

    fn set_text(&self, text: impl Into<Str>) {
        self.set(text.into());
    }
}

#[cfg(feature = "web")]
impl ViewText for web_sys::Text {
    fn new(text: impl Into<Str>) -> Self {
        web_sys::Text::new_with_data(&text.into().inner).unwrap()
    }

    fn set_text(&self, text: impl Into<Str>) {
        let text = text.into();
        self.set_data(&text.inner);
    }
}

#[cfg(feature = "ssr")]
impl ViewText for ssr::Text {
    fn new(text: impl Into<Str>) -> Self {
        ssr::Text::new(text)
    }

    fn set_text(&self, text: impl Into<Str>) {
        ssr::Text::set_text(self, text);
    }
}

pub trait Container {
    type Child;

    fn append_child(&self, child: impl Into<Self::Child>);
}

pub struct TextBuilder<T = web_sys::Text> {
    text: Shared<Str>,
    built: Shared<Option<T>>,
}

impl<T> Clone for TextBuilder<T> {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            built: self.built.clone(),
        }
    }
}

impl<T> TextBuilder<T> {
    pub fn new(text: impl Into<Str>) -> Self {
        Self {
            text: text.into().into(),
            built: Default::default(),
        }
    }
}

pub struct EventListenerBuilder<T> {
    name: Str,
    built: Shared<Option<T>>,
}

/// Builder for runtime views.
///
/// **El** - container element type.
/// **T** - text type.
/// **L** - event listener type.
pub struct ElementBuilder<El, T, L> {
    name: Str,
    built: Shared<Option<El>>,
    attributes: Shared<Vec<(Str, Option<Str>)>>,
    styles: Shared<Vec<(Str, Str)>>,
    events: Shared<Vec<EventListenerBuilder<L>>>,
    children: Shared<Vec<NodeBuilder<El, T, L>>>,
}

impl<El, T, L> Container for ElementBuilder<El, T, L> {
    type Child = NodeBuilder<El, T, L>;

    fn append_child(&self, child: impl Into<Self::Child>) {
        let child = child.into();
        self.children.get_mut().push(child);
    }
}

impl<El, T, L> ElementBuilder<El, T, L> {
    pub fn new(name: impl Into<Str>) -> Self {
        Self {
            name: name.into(),
            built: Default::default(),
            attributes: Default::default(),
            styles: Default::default(),
            events: Default::default(),
            children: Default::default(),
        }
    }
}

pub enum NodeBuilder<El, T, L> {
    Element(ElementBuilder<El, T, L>),
    Text(TextBuilder<T>),
}

impl<El, T, L> From<&TextBuilder<T>> for NodeBuilder<El, T, L> {
    fn from(value: &TextBuilder<T>) -> Self {
        NodeBuilder::Text(value.clone())
    }
}

pub trait View {
    type Element<El, T, L>;
    type Text<T>;
    type EventListener<T>;
}

pub struct Builder;

impl View for Builder {
    type Element<El, T, L> = ElementBuilder<El, T, L>;
    type Text<T> = TextBuilder<T>;
    type EventListener<T> = EventListenerBuilder<T>;
}

pub struct Web;

impl View for Web {
    type Element<El, T, L> = El;
    type Text<T> = T;
    type EventListener<T> = EventListener;
}

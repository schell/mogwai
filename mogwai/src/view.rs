//! Views
use std::{cell::RefCell, rc::Rc};
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget};

use crate::prelude::Receiver;

pub mod builder;
pub mod dom;
pub mod interface;

/// `Effect`s describe a value right now or at many points in the future - or both.
///
/// [`View`]s use `Effect`s to change attributes, styles and inner text.
///
/// An `Effect` can be created from a single value, a [`Receiver`] or a tuple of the
/// two.
pub enum Effect<T> {
    OnceNow { now: T },
    ManyLater { later: Receiver<T> },
    OnceNowAndManyLater { now: T, later: Receiver<T> },
}

impl<T: Clone> Clone for Effect<T> {
    fn clone(&self) -> Self {
        match self {
            Effect::OnceNow { now } => Effect::OnceNow { now: now.clone() },
            Effect::ManyLater { later } => Effect::ManyLater {
                later: later.branch(),
            },
            Effect::OnceNowAndManyLater { now, later } => Effect::OnceNowAndManyLater {
                now: now.clone(),
                later: later.branch(),
            },
        }
    }
}

impl<T> From<Effect<T>> for (Option<T>, Option<Receiver<T>>) {
    fn from(eff: Effect<T>) -> Self {
        match eff {
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

impl From<&String> for Effect<String> {
    fn from(s: &String) -> Effect<String> {
        Effect::OnceNow { now: s.clone() }
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

impl<T> From<(Option<T>, Receiver<Rc<RefCell<Option<T>>>>)> for Effect<Rc<RefCell<Option<T>>>> {
    fn from((now, later): (Option<T>, Receiver<Rc<RefCell<Option<T>>>>)) -> Self {
        let now = Rc::new(RefCell::new(now));
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

/// Marker trait that means JsCast + Clone + + 'static.
pub trait IsDomNode: JsCast + Clone + 'static {}

impl<T> IsDomNode for T where T: JsCast + Clone + 'static {}

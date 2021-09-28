//! Values over time.
use std::sync::{Arc, Mutex};
use crate::{channel::Transmission, Receiver};

/// `Effect`s describe a value right now or at many points in the future - or both.
///
/// `Effect`s are used to change attributes, styles and inner text.
///
/// An `Effect` can be created from either a single value, a [`Receiver`] or a tuple of the
/// two.
pub enum Effect<T> {
    /// A value now.
    OnceNow {
        /// The extant value.
        now: T,
    },
    /// Forthcoming values, to be delivered as messages from a [`Receiver`].
    ManyLater {
        /// The receiver that will deliver new values.
        later: Receiver<T>,
    },
    /// Both a value now and forthcoming values to be delivered as messages from a [`Receiver`].
    OnceNowAndManyLater {
        /// The extant value.
        now: T,
        /// The receiver that will deliver new values.
        later: Receiver<T>,
    },
}

impl<T: Clone + Transmission> Clone for Effect<T> {
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

impl<T> From<(Option<T>, Receiver<Arc<Mutex<Option<T>>>>)> for Effect<Arc<Mutex<Option<T>>>> {
    fn from((now, later): (Option<T>, Receiver<Arc<Mutex<Option<T>>>>)) -> Self {
        let now = Arc::new(Mutex::new(now));
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

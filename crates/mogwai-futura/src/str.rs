//! String type wrapping [`Cow<'static, str>`].
use std::{borrow::Cow, ops::Deref};

use crate::{sync::Shared, view::ViewText};

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

impl Deref for Str {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl ViewText for Shared<Str> {
    fn new(text: impl Into<Str>) -> Self {
        Shared::from(text.into())
    }

    fn set_text(&self, text: impl Into<Str>) {
        self.set(text.into());
    }
}

impl Str {
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

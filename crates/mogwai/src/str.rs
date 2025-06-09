//! String type wrapping [`Cow<'static, str>`].
use std::borrow::Cow;

use crate::{sync::Shared, view::ViewText};

/// An alias for [`Cow<'static, str>`](std::borrow::Cow).
pub type Str = Cow<'static, str>;

impl ViewText for Shared<Str> {
    fn new(text: impl AsRef<str>) -> Self {
        Shared::from_str(text)
    }

    fn set_text(&self, text: impl AsRef<str>) {
        self.set(text.as_ref().to_owned().into());
    }

    fn get_text(&self) -> Str {
        self.get().clone()
    }
}

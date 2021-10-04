//! Wrapped views.
use futures::StreamExt;

/// A wrapper around a domain-specific view.
///
/// If using Mogwai in the browser `T` will most
/// likely be something like `HtmlElement`.
///
/// In general the underlying `T` should be
/// `Clone + Send + Sync + 'static`, but in the
/// browser on WASM we don't worry about `Send + Sync`
/// because that context is single-threaded.
pub struct View<T> {
    /// The underlying domain-specific view type.
    pub inner: T,
}

impl<T: std::fmt::Debug + wasm_bindgen::JsCast + 'static> From<&View<T>> for String {
    fn from(view: &View<T>) -> String {
        if let Some(element) = view.inner.dyn_ref::<web_sys::Element>() {
            return element.outer_html();
        }

        if let Some(text) = view.inner.dyn_ref::<web_sys::Text>() {
            return text.data();
        }
        panic!(
            "Dom reference {:#?} could not be turned into a string",
            view.inner
        );
    }
}

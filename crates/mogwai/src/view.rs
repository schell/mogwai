//! Wrapped views.
use std::{
    convert::TryFrom,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, RwLock, RwLockReadGuard},
};
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Event;

use crate::{
    builder::EventTargetType,
    patch::{HashPatch, ListPatch, ListPatchApply},
    ssr::SsrElement,
    target::Sinking,
};

pub use futures::future::Either;

/// Adds helpful extensions to [`Either`].
pub trait EitherExt {
    /// The left item.
    type LeftItem;

    /// The right item.
    type RightItem;

    /// Return the left item, if possible.
    fn left(self) -> Option<Self::LeftItem>;

    /// Return the left item, if possible.
    fn right(self) -> Option<Self::RightItem>;
}

impl<A, B> EitherExt for Either<A, B> {
    type LeftItem = A;
    type RightItem = B;

    fn left(self) -> Option<Self::LeftItem> {
        match self {
            Either::Left(a) => Some(a),
            Either::Right(_) => None,
        }
    }

    fn right(self) -> Option<Self::RightItem> {
        match self {
            Either::Right(b) => Some(b),
            Either::Left(_) => None,
        }
    }
}

/// A wrapper around a domain-specific view.
pub struct View<T> {
    /// The underlying domain-specific view type.
    pub inner: T,

    #[cfg(target_arch = "wasm32")]
    pub(crate) detach: Arc<RwLock<Box<dyn FnOnce(&T)>>>,

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) detach: Arc<RwLock<Box<dyn FnOnce(&T) + Send + Sync + 'static>>>,
}

impl From<&View<Dom>> for String {
    fn from(view: &View<Dom>) -> String {
        match view.inner.inner_read() {
            Either::Left(val) => {
                if let Some(element) = val.dyn_ref::<web_sys::Element>() {
                    return element.outer_html();
                }

                if let Some(text) = val.dyn_ref::<web_sys::Text>() {
                    return text.data();
                }
                panic!("Dom reference {:#?} could not be turned into a string", val);
            }
            Either::Right(ssr) => {
                let lock = ssr.node.try_lock().unwrap();
                String::from(lock.deref())
            }
        }
    }
}

impl From<View<Dom>> for String {
    fn from(v: View<Dom>) -> Self {
        String::from(&v)
    }
}

impl From<Dom> for View<Dom> {
    fn from(dom: Dom) -> Self {
        View {
            inner: dom,
            detach: Arc::new(RwLock::new(Box::new(|t| t.detach()))),
        }
    }
}

impl View<Dom> {
    /// Run this view in a parent container forever, never dropping it.
    pub fn run_in_container(self, container: &web_sys::Node) -> Result<(), JsValue> {
        let dom = self.into_inner();
        if let Some(node) = dom.clone_as::<web_sys::Node>() {
            let _ = container.append_child(&node);
            Ok(())
        } else {
            Err("running gizmos is only supported on wasm".into())
        }
    }

    /// Run this gizmo in the document body forever, never dropping it.
    pub fn run(self) -> Result<(), JsValue> {
        self.run_in_container(&crate::utils::body())
    }
}

impl<T> Drop for View<T> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.detach) <= 1 {
            let mut lock = self.detach.write().unwrap();
            let detach = std::mem::replace(lock.deref_mut(), Box::new(|_| {}));
            detach(&self.inner);
        }
    }
}

impl<T: Clone> View<T> {
    /// Convert the view into its inner type without detaching the view.
    pub fn into_inner(self) -> T {
        let mut lock = self.detach.write().unwrap();
        *lock = Box::new(|_| {});
        drop(lock);
        self.inner.clone()
    }
}

impl<T> Deref for View<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for View<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A DOM node.
///
/// Represents DOM nodes on WASM and non-WASM targets.
#[derive(Clone)]
pub struct Dom {
    // TODO: This can just be an Arc
    #[cfg(target_arch = "wasm32")]
    node: Arc<RwLock<JsValue>>,
    #[cfg(not(target_arch = "wasm32"))]
    node: SsrElement<web_sys::Event>,
}

impl TryFrom<JsValue> for Dom {
    type Error = JsValue;

    #[cfg(target_arch = "wasm32")]
    fn try_from(node: JsValue) -> Result<Self, Self::Error> {
        Ok(Dom {
            node: Arc::new(RwLock::new(node)),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn try_from(node: JsValue) -> Result<Self, Self::Error> {
        Err(node)
    }
}

impl TryFrom<SsrElement<Event>> for Dom {
    type Error = SsrElement<Event>;

    #[cfg(target_arch = "wasm32")]
    fn try_from(node: SsrElement<Event>) -> Result<Self, Self::Error> {
        Err(node)
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn try_from(node: SsrElement<Event>) -> Result<Self, Self::Error> {
        Ok(Dom { node })
    }
}

impl Dom {
    /// Detaches the node from the DOM.
    pub fn detach(&self) {
        match self.inner_read() {
            Either::Left(val) => {
                if let Some(node) = val.dyn_ref::<web_sys::Node>() {
                    if let Some(parent) = node.parent_node() {
                        let _ = parent.remove_child(&node);
                    }
                }
            }
            Either::Right(_ssr) => {}
        }
    }

    /// Returns a reference of the inner raw node.
    ///
    /// Returns Left(RwReadLockReadGuard<JsValue>) on WASM and Right(&SsrElement) on other.
    ///
    /// This is a helper that prevents you from the requirement of separating your server-side
    /// code from your browser code using cfg.
    #[cfg(target_arch = "wasm32")]
    pub fn inner_read(&self) -> Either<RwLockReadGuard<JsValue>, &SsrElement<web_sys::Event>> {
        let lock = self.node.read().unwrap();
        Either::Left(lock)
    }
    /// Returns a reference of the inner raw node.
    ///
    /// Returns Left(RwReadLockReadGuard<JsValue>) on WASM and Right(&SsrElement) on other.
    ///
    /// This is a helper that prevents you from the requirement of separating your server-side
    /// code from your browser code using cfg.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn inner_read(&self) -> Either<RwLockReadGuard<JsValue>, &SsrElement<web_sys::Event>> {
        Either::Right(&self.node)
    }

    /// Create an element.
    #[cfg(target_arch = "wasm32")]
    pub fn element(tag: &str, namespace: Option<&str>) -> Result<Self, String> {
        let node = Arc::new(RwLock::new(
            if namespace.is_some() {
                crate::utils::document()
                    .create_element_ns(namespace, tag)
                    .map_err(|_| "could not create namespaced element".to_string())
            } else {
                crate::utils::document()
                    .create_element(tag)
                    .map_err(|e| format!("could not create {} element: {:#?}", tag, e))
            }?
            .into(),
        ));
        Ok(Dom { node })
    }
    /// Create an element.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn element(tag: &str, namespace: Option<&str>) -> Result<Self, String> {
        let node = SsrElement::element(tag);
        if namespace.is_some() {
            node.set_attrib("xmlns", namespace)
                .map_err(|_| "not a container".to_string())?;
        }
        Ok(Dom { node })
    }

    /// Create a text node
    #[cfg(target_arch = "wasm32")]
    pub fn text(s: &str) -> Result<Self, String> {
        let text =
            web_sys::Text::new().map_err(|e| format!("could not create wasm text: {:?}", e))?;
        text.set_data(s);
        let node: JsValue = text.into();
        let node = Arc::new(RwLock::new(node));
        Ok(Dom { node })
    }
    /// Create a text node
    #[cfg(not(target_arch = "wasm32"))]
    pub fn text(s: &str) -> Result<Self, String> {
        let node = SsrElement::text(s);
        Ok(Dom { node })
    }

    /// Set the text.
    ///
    /// Fails if this is not a text node.
    pub fn set_text(&self, s: &str) -> Result<(), String> {
        match self.inner_read() {
            Either::Left(val) => {
                val.dyn_ref::<web_sys::Text>()
                    .ok_or_else(|| "not a text node".to_string())?
                    .set_data(s);
            }
            Either::Right(ssr) => {
                ssr.set_text(s).map_err(|_| "not a text node".to_string())?;
            }
        }
        Ok(())
    }

    /// Patch the attributes.
    ///
    /// Fails if this is not a container.
    pub fn patch_attribs(&self, patch: HashPatch<String, String>) -> Result<(), String> {
        match self.inner_read() {
            Either::Left(val) => match patch {
                crate::patch::HashPatch::Insert(k, v) => {
                    val.dyn_ref::<web_sys::Element>()
                        .ok_or_else(|| "not an element".to_string())?
                        .set_attribute(&k, &v)
                        .map_err(|_| "could not set attrib".to_string())?;
                }
                crate::patch::HashPatch::Remove(k) => {
                    val.dyn_ref::<web_sys::Element>()
                        .ok_or_else(|| "not an element".to_string())?
                        .remove_attribute(&k)
                        .map_err(|_| "could remove attrib".to_string())?;
                }
            },
            Either::Right(ssr) => match patch {
                crate::patch::HashPatch::Insert(k, v) => {
                    ssr.set_attrib(&k, Some(&v))
                        .map_err(|_| "could not set attrib".to_string())?;
                }
                crate::patch::HashPatch::Remove(k) => {
                    ssr.remove_attrib(&k)
                        .map_err(|_| "could remove attrib".to_string())?;
                }
            },
        }

        Ok(())
    }

    /// Patch boolean attributes.
    ///
    /// Fails if this is not a container.
    pub fn patch_bool_attribs(&self, patch: HashPatch<String, bool>) -> Result<(), String> {
        match self.inner_read() {
            Either::Left(val) => match patch {
                crate::patch::HashPatch::Insert(k, v) => {
                    if v {
                        val.dyn_ref::<web_sys::Element>()
                            .ok_or_else(|| "not an element".to_string())?
                            .set_attribute(&k, "")
                            .map_err(|_| "could not set boolean attrib".to_string())?;
                    } else {
                        val.dyn_ref::<web_sys::Element>()
                            .ok_or_else(|| "not an element".to_string())?
                            .remove_attribute(&k)
                            .map_err(|_| "could not remove boolean attrib".to_string())?;
                    }
                }
                crate::patch::HashPatch::Remove(k) => {
                    val.dyn_ref::<web_sys::Element>()
                        .ok_or_else(|| "not an element".to_string())?
                        .remove_attribute(&k)
                        .map_err(|_| "could not remove boolean attrib".to_string())?;
                }
            },
            Either::Right(ssr) => match patch {
                crate::patch::HashPatch::Insert(k, v) => {
                    if v {
                        ssr.set_attrib(&k, None)
                            .map_err(|_| "could not set boolean attrib".to_string())?;
                    } else {
                        ssr.remove_attrib(&k)
                            .map_err(|_| "could not remove boolean attrib".to_string())?;
                    }
                }
                crate::patch::HashPatch::Remove(k) => {
                    ssr.remove_attrib(&k)
                        .map_err(|_| "could remove boolean attrib".to_string())?;
                }
            },
        }

        Ok(())
    }

    /// Patch boolean attributes.
    ///
    /// Fails if this is not a container.
    pub fn patch_styles(&self, patch: HashPatch<String, String>) -> Result<(), String> {
        match self.inner_read() {
            Either::Left(val) => {
                let style = val
                    .dyn_ref::<web_sys::HtmlElement>()
                    .ok_or_else(|| "not an element".to_string())?
                    .style();
                match patch {
                    crate::patch::HashPatch::Insert(k, v) => {
                        style
                            .set_property(&k, &v)
                            .map_err(|_| "could not set style".to_string())?;
                    }
                    crate::patch::HashPatch::Remove(k) => {
                        style
                            .remove_property(&k)
                            .map_err(|_| "could not remove style".to_string())?;
                    }
                }
            }
            Either::Right(ssr) => match patch {
                crate::patch::HashPatch::Insert(k, v) => {
                    ssr.set_style(&k, &v)
                        .map_err(|_| "could not set style".to_string())?;
                }
                crate::patch::HashPatch::Remove(k) => {
                    ssr.remove_style(&k)
                        .map_err(|_| "could not remove style".to_string())?;
                }
            },
        }

        Ok(())
    }

    /// Add an event.
    pub fn set_event(&self, type_is: EventTargetType, name: &str, tx: Pin<Box<Sinking<Event>>>) {
        match self.inner_read() {
            Either::Left(val) => match type_is {
                EventTargetType::Myself => {
                    crate::event::add_event(
                        name,
                        val.dyn_ref::<web_sys::EventTarget>()
                            .ok_or_else(|| "not an event target".to_string())
                            .unwrap_throw(),
                        tx,
                    );
                }
                EventTargetType::Window => {
                    crate::event::add_event(name, &web_sys::window().unwrap(), tx);
                }
                EventTargetType::Document => {
                    crate::event::add_event(
                        name,
                        &web_sys::window().unwrap().document().unwrap(),
                        tx,
                    );
                }
            },
            Either::Right(ssr) => ssr.set_event(type_is, name, tx),
        }
    }

    /// Patches child nodes.
    ///
    /// Fails if this is not a container element.
    pub fn patch_children(&self, patch: ListPatch<Self>) -> Result<(), String> {
        match self.inner_read() {
            Either::Left(val) => {
                let patch = patch.map(|d| {
                    d.inner_read()
                        .left()
                        .unwrap()
                        .clone()
                        .dyn_into::<web_sys::Node>()
                        .unwrap()
                });
                val.clone()
                    .dyn_into::<web_sys::Node>()
                    .map_err(|_| "not an element".to_string())?
                    .list_patch_apply(patch);
            }
            Either::Right(ssr) => {
                let patch = patch.map(|d| d.inner_read().right().unwrap().clone());
                ssr.patch_children(patch)
                    .map_err(|_| "not an element".to_string())?;
            }
        }
        Ok(())
    }

    /// Returns a clone of the inner raw node as the given web_sys type, if
    /// possible.
    pub fn clone_as<T: JsCast>(&self) -> Option<T> {
        match self.inner_read() {
            Either::Left(val) => val.clone().dyn_into::<T>().ok(),
            _ => None,
        }
    }

    /// Visits the inner raw node with a function for each target.
    pub fn visit_as<T: JsCast, F, G>(&self, f: F, g: G)
    where
        F: FnOnce(&T),
        G: FnOnce(&SsrElement<Event>),
    {
        match self.inner_read() {
            Either::Left(val) => {
                let el: Option<&T> = val.dyn_ref::<T>();
                el.map(f);
            }
            Either::Right(ssr) => g(ssr),
        }
    }

    /// Attempt to get an attribute value.
    pub fn get_attribute(&self, key: &str) -> Result<Option<String>, String> {
        match self.inner_read() {
            Either::Left(val) => {
                let el = val
                    .dyn_ref::<web_sys::Element>()
                    .ok_or_else(|| "not an Element".to_string())?;
                if el.has_attribute(key) {
                    Ok(el.get_attribute(key))
                } else {
                    Err("no such attribute".to_string())
                }
            }
            Either::Right(ssr) => ssr.get_attrib(key),
        }
    }
}

#[cfg(test)]
mod test {
    fn sendable<T: crate::target::Sendable>() {}

    #[test]
    fn dom_sendable() {
        sendable::<super::Dom>(); // compiles only if true
    }

    #[test]
    fn view_sendable() {
        sendable::<super::View<super::Dom>>(); // compiles only if true
    }
}

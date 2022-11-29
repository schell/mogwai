//! Wrapper around Javascript DOM nodes.
use std::{future::Future, pin::Pin};

use futures::future::Either;
use mogwai::{
    builder::{MogwaiSink, ViewBuilder},
    channel::SinkError,
    futures::{sink::Sink, stream::Stream, EitherExt},
    patch::{HashPatch, ListPatch},
    view::{EventTargetType, View},
};
use wasm_bindgen::{JsCast, JsValue};

use crate::{event::DomEvent, ssr::SsrElement};

/// A wrapper for [`web_sys::Event`].
pub struct JsDomEvent {
    inner: JsValue,
}

/// A Javascript/browser DOM node.
///
/// Represents DOM nodes when a view is built on a WASM target.
///
/// ## Note
/// `JsDom` is !Send and !Sync
#[derive(Clone)]
pub struct JsDom {
    inner: std::sync::Arc<JsValue>,
}

impl View for JsDom {
    type Event = JsDomEvent;
    type FutureType<T> = Pin<Box<dyn Future<Output = T> + Unpin + 'static>>;
    type StreamType<T> = Pin<Box<dyn Stream<Item = T> + Unpin + 'static>>;
    type SinkType<T> = Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + 'static>>;
}

impl From<JsValue> for JsDom {
    fn from(value: JsValue) -> Self {
        JsDom {
            inner: std::sync::Arc::new(value),
        }
    }
}

//impl TryFrom<SsrElement> for JsDom {
//    type Error = SsrElement;
//
//    #[cfg(target_arch = "wasm32")]
//    fn try_from(node: SsrElement) -> Result<Self, Self::Error> {
//        Err(node)
//    }
//    #[cfg(not(target_arch = "wasm32"))]
//    fn try_from(node: SsrElement) -> Result<Self, Self::Error> {
//        Ok(JsDom { node })
//    }
//}

// TODO: Make errors returned by JsDom methods Box<dyn Error>
impl JsDom {
    /// Create a `JsDom` from anything that implements `JsCast`.
    pub fn from_jscast<T: JsCast>(t: &T) -> Self {
        let val = JsValue::from(t);
        JsDom::from(val)
    }

    /// Detaches the node from the DOM.
    pub fn detach(&self) {
        if let Some(node) = self.inner.dyn_ref::<web_sys::Node>() {
            if let Some(parent) = node.parent_node() {
                let _ = parent.remove_child(&node);
            }
        }
    }

    /// Create an element.
    pub fn element(tag: &str, namespace: Option<&str>) -> Result<Self, String> {
        let inner = std::sync::Arc::new(
            if namespace.is_some() {
                crate::utils::document()
                    .unwrap_js::<web_sys::Document>()
                    .create_element_ns(namespace, tag)
                    .map_err(|_| "could not create namespaced element".to_string())
            } else {
                crate::utils::document()
                    .unwrap_js::<web_sys::Document>()
                    .create_element(tag)
                    .map_err(|e| format!("could not create {} element: {:#?}", tag, e))
            }?
            .into(),
        );
        Ok(JsDom { inner })
    }

    ///// Create an element.
    //#[cfg(not(target_arch = "wasm32"))]
    //pub fn element(tag: &str, namespace: Option<&str>) -> Result<Self, String> {
    //    let node = SsrElement::element(tag);
    //    if namespace.is_some() {
    //        node.set_attrib("xmlns", namespace)
    //            .map_err(|_| "not a container".to_string())?;
    //    }
    //    Ok(JsDom { node })
    //}

    /// Create a text node
    pub fn text(s: &str) -> Result<Self, String> {
        let text =
            web_sys::Text::new().map_err(|e| format!("could not create wasm text: {:?}", e))?;
        text.set_data(s);
        let node: JsValue = text.into();
        let inner = std::sync::Arc::new(node);
        Ok(JsDom { inner })
    }

    ///// Create a text node
    //#[cfg(not(target_arch = "wasm32"))]
    //pub fn text(s: &str) -> Result<Self, String> {
    //    let node = SsrElement::text(s);
    //    Ok(JsDom { node })
    //}

    /// Set the text.
    ///
    /// Fails if this is not a text node.
    pub fn set_text(&self, s: &str) -> Result<(), String> {
        self.inner
            .dyn_ref::<web_sys::Text>()
            .ok_or_else(|| "not a text node".to_string())?
            .set_data(s);
        Ok(())
    }

    /// Patch the attributes.
    ///
    /// Fails if this is not a container.
    pub fn patch_attribs(&self, patch: HashPatch<String, String>) -> Result<(), String> {
        match patch {
            HashPatch::Insert(k, v) => {
                self.inner
                    .dyn_ref::<web_sys::Element>()
                    .ok_or_else(|| {
                        format!(
                            "could not set attribute {}={} on {:?}: not an element",
                            k, v, self.inner
                        )
                    })?
                    .set_attribute(&k, &v)
                    .map_err(|_| "could not set attrib".to_string())?;
            }
            HashPatch::Remove(k) => {
                self.inner
                    .dyn_ref::<web_sys::Element>()
                    .ok_or_else(|| {
                        format!("could remove attribute {} on {:?}: not an element", k, self.inner)
                    })?
                    .remove_attribute(&k)
                    .map_err(|_| "could remove attrib".to_string())?;
            }
        }
        //Either::Right(ssr) => match patch {
        //    HashPatch::Insert(k, v) => {
        //        ssr.set_attrib(&k, Some(&v))
        //            .map_err(|_| "could not set attrib".to_string())?;
        //    }
        //    HashPatch::Remove(k) => {
        //        ssr.remove_attrib(&k)
        //            .map_err(|_| "could remove attrib".to_string())?;
        //    }
        //},

        Ok(())
    }

    /// Patch boolean attributes.
    ///
    /// Fails if this is not a container.
    pub fn patch_bool_attribs(&self, patch: HashPatch<String, bool>) -> Result<(), String> {
        match patch {
            HashPatch::Insert(k, v) => {
                if v {
                    self.inner
                        .dyn_ref::<web_sys::Element>()
                        .ok_or_else(|| {
                            format!(
                                "could not set boolean attribute {}={} on {:?}: not an element",
                                k, v, self.inner
                            )
                        })?
                        .set_attribute(&k, "")
                        .map_err(|_| "could not set boolean attrib".to_string())?;
                } else {
                    self.inner
                        .dyn_ref::<web_sys::Element>()
                        .ok_or_else(|| {
                            format!(
                                "could not remove boolean attribute {}={} on {:?}: not an element",
                                k, v, self.inner
                            )
                        })?
                        .remove_attribute(&k)
                        .map_err(|_| "could not remove boolean attrib".to_string())?;
                }
            }
            HashPatch::Remove(k) => {
                self.inner
                    .dyn_ref::<web_sys::Element>()
                    .ok_or_else(|| {
                        format!(
                            "could not remove boolean attribute {} on {:?}: not an element",
                            k, self.inner
                        )
                    })?
                    .remove_attribute(&k)
                    .map_err(|_| "could not remove boolean attrib".to_string())?;
            }
        }
        //HashPatch::Insert(k, v) => {
        //    if v {
        //        ssr.set_attrib(&k, None)
        //            .map_err(|_| "could not set boolean attrib".to_string())?;
        //    } else {
        //        ssr.remove_attrib(&k)
        //            .map_err(|_| "could not remove boolean attrib".to_string())?;
        //    }
        //}
        //HashPatch::Remove(k) => {
        //    ssr.remove_attrib(&k)
        //        .map_err(|_| "could remove boolean attrib".to_string())?;
        //}

        Ok(())
    }

    /// Patch boolean attributes.
    ///
    /// Fails if this is not a container.
    pub fn patch_styles(&self, patch: HashPatch<String, String>) -> Result<(), String> {
        let style = self
            .inner
            .dyn_ref::<web_sys::HtmlElement>()
            .map(|el| el.style())
            .or_else(|| {
                self.inner
                    .dyn_ref::<web_sys::SvgElement>()
                    .map(|el| el.style())
            })
            .ok_or_else(|| format!("could not patch style on {:?}: not an element", self.inner))?;
        match patch {
            HashPatch::Insert(k, v) => {
                style
                    .set_property(&k, &v)
                    .map_err(|_| "could not set style".to_string())?;
            }
            HashPatch::Remove(k) => {
                style
                    .remove_property(&k)
                    .map_err(|_| "could not remove style".to_string())?;
            }
        }
        //Either::Right(ssr) => match patch {
        //    HashPatch::Insert(k, v) => {
        //        ssr.set_style(&k, &v)
        //            .map_err(|_| "could not set style".to_string())?;
        //    }
        //    HashPatch::Remove(k) => {
        //        ssr.remove_style(&k)
        //            .map_err(|_| "could not remove style".to_string())?;
        //    }
        //},

        Ok(())
    }

    /// Add an event.
    pub fn set_event<Si, V>(
        &self,
        type_is: EventTargetType,
        name: &str,
        tx: impl Into<MogwaiSink<web_sys::Event, Si, V>>,
    ) where
        Si: Sink<web_sys::Event, Error = SinkError>,
        V: View,
    {
        use mogwai::futures::sink::Contravariant;
        let sink = tx.into();
        let tx = sink.contra_map(|ev: web_sys::Event| DomEvent::try_from(ev).unwrap());
        match type_is {
            EventTargetType::Myself => {
                crate::event::add_event(
                    name,
                    self.inner
                        .dyn_ref::<web_sys::EventTarget>()
                        .ok_or_else(|| "not an event target".to_string())
                        .unwrap(),
                    tx,
                );
            }
            EventTargetType::Window => {
                crate::event::add_event(name, &web_sys::window().unwrap(), tx);
            }
            EventTargetType::Document => {
                crate::event::add_event(name, &web_sys::window().unwrap().document().unwrap(), tx);
            }
        }
        //#[cfg(not(target_arch = "wasm32"))]
        //{
        //    self.node.set_event(type_is, name, Box::pin(tx));
        //}
    }

    /// Patches child nodes.
    ///
    /// Fails if this is not a container element.
    pub fn patch_children(&self, patch: ListPatch<Self>) -> Result<(), String> {
        match self.inner_read() {
            Either::Left(val) => {
                let patch = patch.map(|d| {
                    mogwai::futures::EitherExt::left(d.inner_read())
                        .unwrap()
                        .clone()
                        .dyn_into::<web_sys::Node>()
                        .unwrap()
                });
                let mut node = val.clone().dyn_into::<web_sys::Node>().map_err(|val| {
                    format!("could not patch children on {:?}: not an element", val)
                })?;
                let _ = super::list_patch_apply_node(&mut node, patch);
            }
            //Either::Right(ssr) => {
            //    let patch = patch.map(|d| d.inner_read().right().unwrap().clone());
            //    ssr.patch_children(patch)
            //        .map_err(|_| "not an element".to_string())?;
            //}
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
    pub fn visit_as<T: JsCast, F, G, A>(&self, f: F, g: G) -> Option<A>
    where
        F: FnOnce(&T) -> A,
        G: FnOnce(&SsrElement) -> A,
    {
        match self.inner_read() {
            Either::Left(val) => {
                let el: Option<&T> = val.dyn_ref::<T>();
                el.map(f)
            }
            Either::Right(ssr) => Some(g(ssr)),
        }
    }

    /// Visites the inner JsCast type with a function.
    ///
    /// ## Panics
    /// Panics if run on any target besides wasm32, or if self cannot be cast
    /// as `T`.
    pub fn visit_js<T: JsCast, A>(&self, f: impl FnOnce(T) -> A) -> A {
        let t = self.clone_as::<T>().unwrap();
        f(t)
    }

    /// Attempt to get an attribute value.
    pub fn get_attribute(&self, key: &str) -> Result<Option<String>, String> {
        match self.inner_read() {
            Either::Left(val) => {
                let el = val.dyn_ref::<web_sys::Element>().ok_or_else(|| {
                    format!(
                        "could not get attribute {} on {:?}: not an Element",
                        key, val
                    )
                })?;
                if el.has_attribute(key) {
                    Ok(el.get_attribute(key))
                } else {
                    Err("no such attribute".to_string())
                }
            }
            Either::Right(ssr) => ssr.get_attrib(key),
        }
    }

    /// Convenience function for converting from any Javascript
    /// value.
    ///
    /// ## Panics
    /// Panics if run on any target but wasm32
    pub fn wrap_js(val: impl Into<JsValue>) -> Self {
        let val: JsValue = val.into();
        JsDom::try_from(val).unwrap()
    }

    /// Convenience function for converting into anything that can be
    /// cast in Javascript.
    ///
    /// ## Panics
    /// Panics if run on any target but wasm32 or if self cannot be cast as
    /// `T`
    pub fn unwrap_js<T: JsCast>(self) -> T {
        self.clone_as::<T>().unwrap()
    }

    /// Return a string representation of the DOM tree.
    ///
    /// ## Panics
    /// Panics if the node cannot be turned into a string representation
    pub async fn html_string(&self) -> String {
        if let Some(element) = self.inner.dyn_ref::<web_sys::Element>() {
            return element.outer_html();
        }

        if let Some(text) = self.inner.dyn_ref::<web_sys::Text>() {
            return text.data();
        }
        panic!("Dom reference {:#?} could not be turned into a string", self.inner);
        //Either::Right(ssr) => ssr.html_string().await,
    }

    /// Run this view in a parent container forever, never dropping it.
    pub fn run_in_container(self, container: &JsDom) -> Result<(), anyhow::Error> {
        let patch = ListPatch::push(self);
        container
            .patch_children(patch)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Run this gizmo in the document body forever, never dropping it.
    pub fn run(self) -> Result<(), anyhow::Error> {
        self.run_in_container(&crate::utils::body())
    }
}

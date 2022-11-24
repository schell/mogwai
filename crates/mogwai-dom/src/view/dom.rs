//! Wrapper around Javascript DOM nodes.

use futures::future::Either;
use mogwai::{
    builder::{MogwaiSink, ViewBuilder},
    futures::{EitherExt, sink::Sink},
    patch::{HashPatch, ListPatch},
    view::{EventTargetType, View},
    traits::ConstraintType
};
use wasm_bindgen::{JsCast, JsValue};

use crate::{event::DomEvent, ssr::SsrElement, view::DomBuilderExt};

/// A DOM node.
///
/// Represents DOM nodes on WASM and non-WASM targets.
#[derive(Clone)]
pub struct Dom {
    #[cfg(target_arch = "wasm32")]
    node: std::sync::Arc<JsValue>,
    #[cfg(not(target_arch = "wasm32"))]
    node: SsrElement,
}

impl View for Dom {
    type Event = DomEvent;
}

impl TryFrom<JsValue> for Dom {
    type Error = JsValue;

    #[cfg(target_arch = "wasm32")]
    fn try_from(node: JsValue) -> Result<Self, Self::Error> {
        Ok(Dom {
            node: std::sync::Arc::new(node),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn try_from(node: JsValue) -> Result<Self, Self::Error> {
        Err(node)
    }
}

impl TryFrom<SsrElement> for Dom {
    type Error = SsrElement;

    #[cfg(target_arch = "wasm32")]
    fn try_from(node: SsrElement) -> Result<Self, Self::Error> {
        Err(node)
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn try_from(node: SsrElement) -> Result<Self, Self::Error> {
        Ok(Dom { node })
    }
}

impl Dom {
    /// Attempt to create a `Dom` from any `JsCast`.
    ///
    /// If the conversion fails you get the original value back.
    pub fn from_jscast<T: JsCast>(t: &T) -> Result<Self, T> {
        let val = JsValue::from(t);
        Dom::try_from(val).map_err(|val| val.dyn_into::<T>().unwrap())
    }

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
            Either::Right(_ssr) => {
                // TODO: detach for SSR
                todo!("no detach for SSR");
            }
        }
    }

    /// Returns a reference of the inner raw node.
    ///
    /// Returns Left(RwReadLockReadGuard<JsValue>) on WASM and Right(&SsrElement) on other.
    ///
    /// This is a helper that prevents you from the requirement of separating your server-side
    /// code from your browser code using cfg.
    pub fn inner_read(&self) -> Either<&JsValue, &SsrElement> {
        #[cfg(target_arch = "wasm32")]
        {
            Either::Left(&self.node)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Either::Right(&self.node)
        }
    }

    /// Create an element.
    #[cfg(target_arch = "wasm32")]
    pub fn element(tag: &str, namespace: Option<&str>) -> Result<Self, String> {
        let node = std::sync::Arc::new(
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
        let node = std::sync::Arc::new(node);
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
                HashPatch::Insert(k, v) => {
                    val.dyn_ref::<web_sys::Element>()
                        .ok_or_else(|| {
                            format!(
                                "could not set attribute {}={} on {:?}: not an element",
                                k, v, val
                            )
                        })?
                        .set_attribute(&k, &v)
                        .map_err(|_| "could not set attrib".to_string())?;
                }
                HashPatch::Remove(k) => {
                    val.dyn_ref::<web_sys::Element>()
                        .ok_or_else(|| {
                            format!("could remove attribute {} on {:?}: not an element", k, val)
                        })?
                        .remove_attribute(&k)
                        .map_err(|_| "could remove attrib".to_string())?;
                }
            },
            Either::Right(ssr) => match patch {
                HashPatch::Insert(k, v) => {
                    ssr.set_attrib(&k, Some(&v))
                        .map_err(|_| "could not set attrib".to_string())?;
                }
                HashPatch::Remove(k) => {
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
            Either::Left(val) => {
                match patch {
                    HashPatch::Insert(k, v) => {
                        if v {
                            val.dyn_ref::<web_sys::Element>()
                            .ok_or_else(|| format!("could not set boolean attribute {}={} on {:?}: not an element", k, v, val))?
                            .set_attribute(&k, "")
                            .map_err(|_| "could not set boolean attrib".to_string())?;
                        } else {
                            val.dyn_ref::<web_sys::Element>()
                            .ok_or_else(|| format!("could not remove boolean attribute {}={} on {:?}: not an element", k, v, val))?
                            .remove_attribute(&k)
                            .map_err(|_| "could not remove boolean attrib".to_string())?;
                        }
                    }
                    HashPatch::Remove(k) => {
                        val.dyn_ref::<web_sys::Element>()
                            .ok_or_else(|| {
                                format!(
                                    "could not remove boolean attribute {} on {:?}: not an element",
                                    k, val
                                )
                            })?
                            .remove_attribute(&k)
                            .map_err(|_| "could not remove boolean attrib".to_string())?;
                    }
                }
            }
            Either::Right(ssr) => match patch {
                HashPatch::Insert(k, v) => {
                    if v {
                        ssr.set_attrib(&k, None)
                            .map_err(|_| "could not set boolean attrib".to_string())?;
                    } else {
                        ssr.remove_attrib(&k)
                            .map_err(|_| "could not remove boolean attrib".to_string())?;
                    }
                }
                HashPatch::Remove(k) => {
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
                    .map(|el| el.style())
                    .or_else(|| val.dyn_ref::<web_sys::SvgElement>().map(|el| el.style()))
                    .ok_or_else(|| format!("could not patch style on {:?}: not an element", val))?;
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
            }
            Either::Right(ssr) => match patch {
                HashPatch::Insert(k, v) => {
                    ssr.set_style(&k, &v)
                        .map_err(|_| "could not set style".to_string())?;
                }
                HashPatch::Remove(k) => {
                    ssr.remove_style(&k)
                        .map_err(|_| "could not remove style".to_string())?;
                }
            },
        }

        Ok(())
    }

    /// Add an event.
    pub fn set_event<Si: Sink<DomEvent>, C: ConstraintType>(
        &self,
        type_is: EventTargetType,
        name: &str,
        tx: impl Into<MogwaiSink<DomEvent, Si, C>>,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            use mogwai_core::futures::sink::Contravariant;
            let tx = Box::pin(tx.contra_map(|ev: web_sys::Event| DomEvent::try_from(ev).unwrap()));
            match type_is {
                EventTargetType::Myself => {
                    crate::event::add_event(
                        name,
                        self.node
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
                    crate::event::add_event(
                        name,
                        &web_sys::window().unwrap().document().unwrap(),
                        tx,
                    );
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.node.set_event(type_is, name, Box::pin(tx));
        }
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
            Either::Right(ssr) => {
                let patch = patch.map(|d| d.inner_read().right().unwrap().clone());
                ssr.patch_children(patch)
                    .map_err(|_| "not an element".to_string())?;
            }
        }
        Ok(())
    }

    /// Builds and patches nodes asynchronously.
    ///
    /// Fails if this is not a container element or if the patch fails.
    pub fn build_and_patch_children(
        &self,
        patch: ListPatch<ViewBuilder<Self, C>>,
    ) -> Result<(), anyhow::Error> {
        let patch = patch.map(|builder: ViewBuilder<Dom>| -> Dom { builder.build().unwrap() });
        self.patch_children(patch)
            .map_err(|_| anyhow::anyhow!("could not build and patch"))
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
        Dom::try_from(val).unwrap()
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
    pub async fn html_string(&self) -> String {
        match self.inner_read() {
            Either::Left(val) => {
                if let Some(element) = val.dyn_ref::<web_sys::Element>() {
                    return element.outer_html();
                }

                if let Some(text) = val.dyn_ref::<web_sys::Text>() {
                    return text.data();
                }
                panic!("Dom reference {:#?} could not be turned into a string", val);
            }
            Either::Right(ssr) => ssr.html_string().await,
        }
    }

    /// Run this view in a parent container forever, never dropping it.
    pub fn run_in_container(self, container: &Dom) -> Result<(), anyhow::Error> {
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

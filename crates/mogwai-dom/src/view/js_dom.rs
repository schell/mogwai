//! Wrapper around Javascript DOM nodes.
use std::{
    future::Future,
    ops::{Bound, RangeBounds, Deref},
};

use anyhow::Context;
use mogwai::{
    patch::{HashPatch, ListPatch},
    view::{AnyEvent, AnyView, Update, View, ViewBuilder, ViewIdentity, ViewResources},
};
use send_wrapper::SendWrapper;
use wasm_bindgen::{JsCast, JsValue};

use crate::event::JsDomEvent;

/// An empty type because we don't need anything but static references to build browser DOM.
pub struct JsDomResources;

impl ViewResources<JsDom> for JsDomResources {
    fn init(&self, identity: ViewIdentity) -> anyhow::Result<JsDom> {
        match identity {
            ViewIdentity::Branch(tag) => JsDom::element(&tag, None),
            ViewIdentity::NamespacedBranch(tag, ns) => JsDom::element(&tag, Some(&ns)),
            ViewIdentity::Leaf(text) => JsDom::text(&text),
        }
    }

    fn spawn(&self, action: impl Future<Output = ()> + Send + 'static) {
        wasm_bindgen_futures::spawn_local(action)
    }
}

/// A Javascript/browser DOM node.
///
/// Represents DOM nodes when a view is built on a WASM target.
#[derive(Clone)]
pub struct JsDom {
    inner: SendWrapper<std::sync::Arc<JsValue>>,
}

impl Deref for JsDom {
    type Target = JsValue;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl From<JsValue> for JsDom {
    fn from(value: JsValue) -> Self {
        JsDom {
            inner: SendWrapper::new(std::sync::Arc::new(value)),
        }
    }
}

impl View for JsDom {
    fn update(&self, update: Update) -> anyhow::Result<()> {
        match update {
            Update::Text(s) => {
                self.inner
                    .dyn_ref::<web_sys::Text>()
                    .context("not a text node")?
                    .set_data(&s);
            }
            Update::Attribute(patch) => match patch {
                HashPatch::Insert(k, v) => {
                    self.inner
                        .dyn_ref::<web_sys::Element>()
                        .with_context(|| {
                            format!(
                                "could not set attribute {}={} on {:?}: not an element",
                                k, v, self.inner
                            )
                        })?
                        .set_attribute(&k, &v)
                        .map_err(|_| anyhow::anyhow!("could not set attrib"))?;
                }
                HashPatch::Remove(k) => {
                    self.inner
                        .dyn_ref::<web_sys::Element>()
                        .with_context(|| {
                            format!(
                                "could remove attribute {} on {:?}: not an element",
                                k, self.inner
                            )
                        })?
                        .remove_attribute(&k)
                        .map_err(|_| anyhow::anyhow!("could remove attrib"))?;
                }
            },
            Update::BooleanAttribute(patch) => match patch {
                HashPatch::Insert(k, v) => {
                    if v {
                        self.inner
                            .dyn_ref::<web_sys::Element>()
                            .with_context(|| {
                                format!(
                                    "could not set boolean attribute {}={} on {:?}: not an element",
                                    k, v, self.inner
                                )
                            })?
                            .set_attribute(&k, "")
                            .map_err(|_| anyhow::anyhow!("could not set boolean attrib"))?;
                    } else {
                        self.inner
                            .dyn_ref::<web_sys::Element>()
                            .with_context(|| {
                                format!(
                                "could not remove boolean attribute {}={} on {:?}: not an element",
                                k, v, self.inner
                            )
                            })?
                            .remove_attribute(&k)
                            .map_err(|_| anyhow::anyhow!("could not remove boolean attrib"))?;
                    }
                }
                HashPatch::Remove(k) => {
                    self.inner
                        .dyn_ref::<web_sys::Element>()
                        .with_context(|| {
                            format!(
                                "could not remove boolean attribute {} on {:?}: not an element",
                                k, self.inner
                            )
                        })?
                        .remove_attribute(&k)
                        .map_err(|_| {
                            anyhow::anyhow!("could not remove boolean attrib".to_string())
                        })?;
                }
            },
            Update::Style(patch) => {
                let style = self
                    .inner
                    .dyn_ref::<web_sys::HtmlElement>()
                    .map(|el| el.style())
                    .or_else(|| {
                        self.inner
                            .dyn_ref::<web_sys::SvgElement>()
                            .map(|el| el.style())
                    })
                    .with_context(|| {
                        format!("could not patch style on {:?}: not an element", self.inner)
                    })?;
                match patch {
                    HashPatch::Insert(k, v) => {
                        style
                            .set_property(&k, &v)
                            .map_err(|_| anyhow::anyhow!("could not set style"))?;
                    }
                    HashPatch::Remove(k) => {
                        style
                            .remove_property(&k)
                            .map_err(|_| anyhow::anyhow!("could not remove style"))?;
                    }
                }
            }
            Update::Child(patch) => {
                let patch: ListPatch<web_sys::Node> =
                    patch.try_map(|builder: ViewBuilder| -> anyhow::Result<web_sys::Node> {
                        let child: JsDom = builder.try_into()?;
                        child
                            .inner
                            .dyn_ref::<web_sys::Node>()
                            .cloned()
                            .context("not a dom node")
                    })?;
                let mut node = self
                    .inner
                    .dyn_ref::<web_sys::Node>()
                    .cloned()
                    .context("could not patch children parent is not an element")?;
                let _ = list_patch_apply_node(&mut node, patch);
            }
            Update::Listener {
                event_name,
                event_target,
                sink,
            } => {
                use mogwai::futures::sink::Contravariant;
                let tx =
                    sink.contra_map(|event: web_sys::Event| AnyEvent::new(JsDomEvent::from(event)));
                match event_target.as_str() {
                    "myself" => {
                        crate::event::add_event::<JsDom>(
                            &event_name,
                            self.inner
                                .dyn_ref::<web_sys::EventTarget>()
                                .ok_or_else(|| "not an event target".to_string())
                                .unwrap(),
                            Box::pin(tx),
                        );
                    }
                    "window" => {
                        crate::event::add_event::<JsDom>(
                            &event_name,
                            &web_sys::window().unwrap(),
                            Box::pin(tx),
                        );
                    }
                    "document" => {
                        crate::event::add_event::<JsDom>(
                            &event_name,
                            &web_sys::window().unwrap().document().unwrap(),
                            Box::pin(tx),
                        );
                    }
                    _ => anyhow::bail!("unsupported event target {}", event_target),
                }
            }
            Update::PostBuild(f) => {
                let node = self.clone();
                (f)(AnyView::new(node))?;
            }
        }

        Ok(())
    }

    //fn spawn(&self, action: impl Future<Output = ()> + Send + 'static) {
    //    wasm_bindgen_futures::spawn_local(action)
    //}
}

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
    pub fn element(tag: &str, namespace: Option<&str>) -> anyhow::Result<Self> {
        let inner = SendWrapper::new(std::sync::Arc::new(
            if namespace.is_some() {
                crate::utils::document()
                    .clone_as::<web_sys::Document>()
                    .context("not document")?
                    .create_element_ns(namespace, tag)
                    .map_err(|v| anyhow::anyhow!("could not create namespaced element: {:?}", v))
            } else {
                crate::utils::document()
                    .clone_as::<web_sys::Document>()
                    .context("not document")?
                    .create_element(tag)
                    .map_err(|e| anyhow::anyhow!("could not create {} element: {:#?}", tag, e))
            }?
            .into(),
        ));
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
    pub fn text(s: &str) -> anyhow::Result<Self> {
        let text = web_sys::Text::new()
            .map_err(|e| anyhow::anyhow!("could not create wasm text: {:?}", e))?;
        text.set_data(s);
        let node: JsValue = text.into();
        let inner = SendWrapper::new(std::sync::Arc::new(node));
        Ok(JsDom { inner })
    }

    ///// Create a text node
    //#[cfg(not(target_arch = "wasm32"))]
    //pub fn text(s: &str) -> Result<Self, String> {
    //    let node = SsrElement::text(s);
    //    Ok(JsDom { node })
    //}

    /// Returns a clone of the inner raw node as the given web_sys type, if
    /// possible.
    pub fn clone_as<T: JsCast + Clone>(&self) -> Option<T> {
        self.inner.dyn_ref::<T>().cloned()
    }

    ///// Visits the inner raw node with a function for each target.
    //pub fn visit_as<T: JsCast, F, G, A>(&self, f: F, g: G) -> Option<A>
    //where
    //    F: FnOnce(&T) -> A,
    //{
    //    let el: Option<&T> = self.inner.dyn_ref::<T>();
    //    el.map(f)
    //    //    Either::Right(ssr) => Some(g(ssr)),
    //    //}
    //}

    ///// Visites the inner JsCast type with a function.
    /////
    ///// ## Panics
    ///// Panics if run on any target besides wasm32, or if self cannot be cast
    ///// as `T`.
    //pub fn visit_js<T: JsCast, A>(&self, f: impl FnOnce(T) -> A) -> A {
    //    let t = self.clone_as::<T>().unwrap();
    //    f(t)
    //}

    ///// Attempt to get an attribute value.
    //pub fn get_attribute(&self, key: &str) -> Result<Option<String>, String> {
    //    match self.inner_read() {
    //        Either::Left(val) => {
    //            let el = val.dyn_ref::<web_sys::Element>().ok_or_else(|| {
    //                format!(
    //                    "could not get attribute {} on {:?}: not an Element",
    //                    key, val
    //                )
    //            })?;
    //            if el.has_attribute(key) {
    //                Ok(el.get_attribute(key))
    //            } else {
    //                Err("no such attribute".to_string())
    //            }
    //        }
    //        Either::Right(ssr) => ssr.get_attrib(key),
    //    }
    //}

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
        panic!(
            "Dom reference {:#?} could not be turned into a string",
            self.inner
        );
        //Either::Right(ssr) => ssr.html_string().await,
    }

    /// Run this view in a parent container forever, never dropping it.
    pub fn run_in_container(self, container: &JsDom) -> anyhow::Result<()> {
        let node: web_sys::Node = self
            .inner
            .dyn_ref::<web_sys::Node>()
            .context("could not downcast to Node")?
            .clone();
        let mut container_node: web_sys::Node = container
            .inner
            .dyn_ref::<web_sys::Node>()
            .context("could not downcast to Node")?
            .clone();
        let patch = ListPatch::push(node);
        let _ = list_patch_apply_node(&mut container_node, patch);
        Ok(())
    }

    /// Run this gizmo in the document body forever, never dropping it.
    pub fn run(self) -> Result<(), anyhow::Error> {
        self.run_in_container(&crate::utils::body())
    }
}

// Helper function for defining `ListPatchApply for JsDom`.
fn list_patch_apply_node(
    self_node: &mut web_sys::Node,
    patch: ListPatch<web_sys::Node>,
) -> Vec<web_sys::Node> {
    let mut removed = vec![];
    match patch {
        ListPatch::Splice {
            range,
            replace_with,
        } => {
            let mut replace_with = replace_with.into_iter();
            let list: web_sys::NodeList = self_node.child_nodes();
            let children: Vec<web_sys::Node> =
                (0..list.length()).filter_map(|i| list.get(i)).collect();

            let start_index = match range.0 {
                Bound::Included(i) => i,
                Bound::Excluded(i) => i,
                Bound::Unbounded => 0,
            };
            let end_index = match range.1 {
                Bound::Included(i) => i,
                Bound::Excluded(i) => i,
                Bound::Unbounded => (list.length() as usize).max(1) - 1,
            };

            let mut child_after = None;
            for i in start_index..=end_index {
                if let Some(old_child) = children.get(i) {
                    if range.contains(&i) {
                        if let Some(new_child) = replace_with.next() {
                            self_node.replace_child(&new_child, &old_child).unwrap();
                        } else {
                            self_node.remove_child(&old_child).unwrap();
                        }
                        removed.push(old_child.clone());
                    } else {
                        child_after = Some(old_child);
                    }
                }
            }

            for child in replace_with {
                self_node.insert_before(&child, child_after).unwrap();
            }
        }
        ListPatch::Push(new_node) => {
            let _ = self_node.append_child(&new_node).unwrap();
        }
        ListPatch::Pop => {
            if let Some(child) = self_node.last_child() {
                let _ = self_node.remove_child(&child).unwrap();
                removed.push(child);
            }
        }
    }
    removed
}

impl TryFrom<ViewBuilder> for JsDom {
    type Error = anyhow::Error;

    fn try_from(value: ViewBuilder) -> Result<Self, Self::Error> {
        JsDomResources.build(value)
    }
}

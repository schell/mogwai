//! Wrapper around Javascript DOM nodes.
use std::{
    collections::HashMap,
    future::Future,
    ops::{Bound, Deref, RangeBounds},
    pin::Pin,
    sync::{
        Arc, Weak
    },
    task::Waker,
};

use anyhow::Context;
use async_lock::RwLock;
use mogwai::{
    channel::mpsc,
    patch::{HashPatch, HashPatchApply, ListPatch, ListPatchApply},
    sink::SinkExt,
    stream::{Stream, StreamExt},
    view::{AnyEvent, AnyView, Downcast, Listener, Update, ViewBuilder, ViewIdentity},
};
use send_wrapper::SendWrapper;
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};

use crate::{
    event::{JsDomEvent, WebCallback},
    prelude::{DOCUMENT, WINDOW},
};

use super::{atomic::AtomicOption, FutureTask};

#[derive(Debug)]
pub(crate) struct Shared<T>(Arc<T>);

impl<T: Default> Default for Shared<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T> AsRef<T> for Shared<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> Shared<T> {
    pub(crate) fn new(t:T) -> Self {
        Self(Arc::new(t))
    }

    pub(crate) fn downgrade(&self) -> WeakShared<T> {
        WeakShared(Arc::downgrade(&self.0))
    }

    pub(crate) fn strong_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }
}

#[derive(Debug)]
pub(crate) struct WeakShared<T>(Weak<T>);

impl<T> Clone for WeakShared<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

struct CancelStream<St> {
    st: St,
    waker: Shared<AtomicOption<Waker>>,
}

impl<St: Stream + Unpin> Stream for CancelStream<St> {
    type Item = St::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.is_cancelled() {
            std::task::Poll::Ready(None)
        } else {
            self.waker.swap(Some(cx.waker().clone()));
            self.get_mut().st.poll_next(cx)
        }
    }
}

impl<St> CancelStream<St> {
    fn is_cancelled(&self) -> bool {
        self.waker.strong_count() < 2
    }
}

#[derive(Clone)]
pub(crate) struct StreamHandle {
    waker: Shared<AtomicOption<Waker>>,
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        if let Some(waker) = self.waker.as_ref().take() {
            waker.wake();
        }
    }
}

fn stream_and_handle<St>(st: St) -> (CancelStream<St>, StreamHandle) {
    let waker = Shared::new(AtomicOption::new(None));
    (
        CancelStream {
            waker: waker.clone(),
            st,
        },
        StreamHandle { waker },
    )
}

pub fn spawn_local(future: impl Future<Output = ()> + Send + Unpin + 'static) {
    wasm_bindgen_futures::spawn_local(future)
}

/// A Javascript/browser DOM node.
///
/// Represents DOM nodes when a view is built on a WASM target.
#[derive(Clone)]
pub struct JsDom {
    pub(crate) inner: SendWrapper<JsValue>,
    pub(crate) update_handle: Option<StreamHandle>,
    pub(crate) listener_callbacks: Shared<RwLock<Vec<WebCallback>>>,
    pub(crate) children: Shared<RwLock<Vec<JsDom>>>,
    // a list of this element's parent's children, so that this element may remove itself
    pub(crate) parents_children: Option<WeakShared<RwLock<Vec<JsDom>>>>,
}

impl Downcast<JsDom> for AnyView {
    fn downcast(self) -> anyhow::Result<JsDom> {
        #[cfg(debug_assertions)]
        let type_name = self.inner_type_name;
        #[cfg(not(debug_assertions))]
        let type_name = "unknown";

        let v: Box<JsDom> = self
            .inner
            .downcast()
            .ok()
            .with_context(|| format!("could not downcast AnyView{{{type_name}}} to JsDom",))?;
        Ok(*v)
    }
}

impl std::fmt::Debug for JsDom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsDom")
            .field("inner", &self.inner)
            .field("children", &self.children)
            .finish()
    }
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
            inner: SendWrapper::new(value),
            update_handle: Default::default(),
            listener_callbacks: Default::default(),
            children: Default::default(),
            parents_children: None,
        }
    }
}

// TODO: Make errors returned by JsDom methods Box<dyn Error>
impl JsDom {
    /// Create a `JsDom` from anything that implements `JsCast`.
    pub fn from_jscast<T: JsCast>(t: &T) -> Self {
        let val = JsValue::from(t);
        JsDom::from(val)
    }

    /// Given a setter function on a `JsCast` type, return another setter
    /// function that takes `Self` and sets a value on it, if possible.
    ///
    /// If `Self` cannot be used as the given `JsCast` type, the returned setter
    /// function will log an error.
    ///
    /// This is useful in conjunction with the `capture:for_each`
    /// [`rsx`](crate::rsx) macro attribute.
    /// See [`ViewBuilder::with_capture_for_each`] for more details.
    pub fn try_to<E: JsCast, S: ?Sized, T: AsRef<S>>(
        f: impl Fn(&E, &S) + Send + 'static,
    ) -> Box<dyn Fn(&Self, T) + Send + 'static> {
        Box::new(move |js: &JsDom, t: T| {
            let res = js.visit_as::<E, ()>(|el| f(el, t.as_ref()));
            if res.is_none() {
                log::error!("could not use {:?} as {}", js, std::any::type_name::<E>());
            }
        })
    }

    pub fn update(&self, update: Update) -> anyhow::Result<()> {
        match update {
            Update::Text(s) => {
                self.inner
                    .unchecked_ref::<web_sys::Text>()
                    .set_data(&s);
            }
            Update::Attribute(patch) => match patch {
                HashPatch::Insert(k, v) => {
                    self.inner
                        .unchecked_ref::<web_sys::Element>()
                        .set_attribute(&k, &v)
                        .map_err(|_| anyhow::anyhow!("could not set attrib"))?;
                }
                HashPatch::Remove(k) => {
                    self.inner
                        .unchecked_ref::<web_sys::Element>()
                        .remove_attribute(&k)
                        .map_err(|_| anyhow::anyhow!("could remove attrib"))?;
                }
            },
            Update::BooleanAttribute(patch) => match patch {
                HashPatch::Insert(k, v) => {
                    if v {
                        self.inner
                            .unchecked_ref::<web_sys::Element>()
                            .set_attribute(&k, "")
                            .map_err(|_| anyhow::anyhow!("could not set boolean attrib"))?;
                    } else {
                        self.inner
                            .unchecked_ref::<web_sys::Element>()
                            .remove_attribute(&k)
                            .map_err(|_| anyhow::anyhow!("could not remove boolean attrib"))?;
                    }
                }
                HashPatch::Remove(k) => {
                    self.inner
                        .unchecked_ref::<web_sys::Element>()
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
                let patch: ListPatch<JsDom> = patch.try_map(JsDom::try_from)?;
                let _ = self.patch(patch);
            }
        }

        Ok(())
    }

    /// Detaches the node from the DOM.
    pub fn detach(&self) {
        let node = self.inner.unchecked_ref::<web_sys::Node>();
        if let Some(parent) = node.parent_node() {
            let _ = parent.remove_child(&node);
        }
    }

    /// Create an element.
    pub fn element(tag: &str, namespace: Option<&str>) -> anyhow::Result<Self> {
        let inner = SendWrapper::new(
            if namespace.is_some() {
                DOCUMENT.with(|d| {
                    d.create_element_ns(namespace, tag).map_err(|v| {
                        anyhow::anyhow!("could not create namespaced element: {:?}", v)
                    })
                })
            } else {
                DOCUMENT.with(|d| {
                    d.create_element(tag)
                        .map_err(|e| anyhow::anyhow!("could not create {} element: {:#?}", tag, e))
                })
            }?
            .into(),
        );
        Ok(JsDom {
            inner,
            update_handle: Default::default(),
            listener_callbacks: Default::default(),
            children: Default::default(),
            parents_children: None,
        })
    }

    /// Create a text node
    pub fn text(s: &str) -> anyhow::Result<Self> {
        let text = web_sys::Text::new()
            .map_err(|e| anyhow::anyhow!("could not create wasm text: {:?}", e))?;
        text.set_data(s);
        let node: JsValue = text.into();
        let inner = SendWrapper::new(node);
        Ok(JsDom {
            inner,
            update_handle: Default::default(),
            listener_callbacks: Default::default(),
            children: Default::default(),
            parents_children: None,
        })
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

    /// Visits the inner node with a function, if the node can be cast
    /// correctly.
    pub fn visit_as<T: JsCast, A>(&self, f: impl FnOnce(&T) -> A) -> Option<A> {
        let el: &T = self.inner.dyn_ref::<T>()?;
        Some(f(el))
    }

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

    pub fn patch(&self, patch: ListPatch<JsDom>) -> Vec<JsDom> {
        let node_patch = patch
            .clone()
            .map(|js| js.clone_as::<web_sys::Node>().unwrap_throw());

        let mut parent = self.inner.unchecked_ref::<web_sys::Node>().clone();
        list_patch_apply_node(&mut parent, node_patch);

        let weakly_shared_children = Some(self.children.downgrade());
        let mut w = self.children.try_write().unwrap_throw();
        let mut removed = w.list_patch_apply(patch.map(|mut js_dom| {
            js_dom.parents_children = weakly_shared_children.clone();
            js_dom
        }));
        for removed_child in removed.iter_mut() {
            removed_child.parents_children = None;
        }
        removed
    }

    /// Run this view in a parent container forever, never dropping it.
    pub fn run_in_container(self, container: JsDom) -> anyhow::Result<()> {
        container.patch(ListPatch::push(self));
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                crate::core::time::wait_millis(10_000).await;
                let _ = &container;
            }
        });
        Ok(())
    }

    /// Run this gizmo in the document body forever, never dropping it.
    pub fn run(self) -> anyhow::Result<()> {
        self.run_in_container(crate::utils::body())
    }

    pub async fn run_while<T: 'static>(
        &self,
        fut: impl Future<Output = T> + 'static,
    ) -> anyhow::Result<T> {
        let (tx, mut rx) = mpsc::bounded(1);
        wasm_bindgen_futures::spawn_local(async move {
            let t = fut.await;
            let _ = tx.send(t).await.unwrap_throw();
        });
        let t = rx.next().await.context("future never finished")?;
        Ok(t)
    }

    /// Add an event listener to this element.
    pub fn add_listener(
        &self,
        Listener {
            event_name,
            event_target,
            sink,
        }: Listener,
    ) -> anyhow::Result<()> {
        let tx = sink.contra_map(|event: JsDomEvent| AnyEvent::new(event));
        let callback = match event_target {
            "myself" => crate::event::add_event(
                &event_name,
                self.inner
                    .dyn_ref::<web_sys::EventTarget>()
                    .ok_or_else(|| "not an event target".to_string())
                    .unwrap_throw(),
                Box::pin(tx),
            ),
            "window" => {
                crate::event::add_event(&event_name, &WINDOW.with(|w| w.clone()), Box::pin(tx))
            }
            "document" => {
                crate::event::add_event(&event_name, &DOCUMENT.with(|d| d.clone()), Box::pin(tx))
            }
            _ => anyhow::bail!("unsupported event target {}", event_target),
        };
        let mut write = self
            .listener_callbacks
            .try_write()
            .context("cannot acquire write")?;
        write.push(callback);

        Ok(())
    }

    pub fn ossify(self) -> JsDom {
        let element: JsValue = (*self.inner).clone();
        JsDom::from(element)
    }

    pub fn hydrate(&self, mut builder: ViewBuilder) -> anyhow::Result<JsDom> {
        builder.hydration_root = Some(AnyView::new(self.clone()));
        Hydrator::try_from(builder).map(|h| h.inner)
    }
}

// Helper function for defining `ListPatchApply for JsDom`.
pub(crate) fn list_patch_apply_node(
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
                            self_node.replace_child(&new_child, &old_child).unwrap_throw();
                        } else {
                            self_node.remove_child(&old_child).unwrap_throw();
                        }
                        removed.push(old_child.clone());
                    } else {
                        child_after = Some(old_child);
                    }
                }
            }

            for child in replace_with {
                self_node.insert_before(&child, child_after).unwrap_throw();
            }
        }
        ListPatch::Push(new_node) => {
            let _ = self_node.append_child(&new_node).unwrap_throw();
        }
        ListPatch::Pop => {
            if let Some(child) = self_node.last_child() {
                let _ = self_node.remove_child(&child).unwrap_throw();
                removed.push(child);
            }
        }
        ListPatch::Noop => {}
    }
    removed
}

impl ListPatchApply for JsDom {
    type Item = JsDom;

    fn list_patch_apply(&mut self, patch: ListPatch<JsDom>) -> Vec<JsDom> {
        self.patch(patch)
    }
}

pub(crate) fn build(
    builder: ViewBuilder,
    may_parent: Option<(usize, &web_sys::Node)>,
) -> anyhow::Result<JsDom> {
    let ViewBuilder {
        identity,
        initial_values,
        updates,
        post_build_ops,
        view_sinks,
        listeners,
        tasks,
        hydration_root,
    } = builder;
    let hydrating_root = hydration_root.is_some();
    let hydrating_child = may_parent.is_some();

    // intialize it
    let mut dom = if hydrating_child {
        let attribs = initial_values.iter().filter_map(|update| match update {
            Update::Attribute(patch) => Some(patch.clone()),
            _ => None,
        });
        let key = match identity {
            ViewIdentity::Branch(t) => HydrationKey::try_new(t, attribs, may_parent),
            ViewIdentity::NamespacedBranch(t, _) => HydrationKey::try_new(t, attribs, may_parent),
            ViewIdentity::Leaf(t) => HydrationKey::try_new(t, attribs, may_parent),
        }?;
        key.hydrate()?
    } else {
        if let Some(root) = hydration_root {
            root.downcast()?
        } else {
            match identity {
                ViewIdentity::Branch(tag) => JsDom::element(&tag, None),
                ViewIdentity::NamespacedBranch(tag, ns) => JsDom::element(&tag, Some(&ns)),
                ViewIdentity::Leaf(text) => JsDom::text(&text),
            }?
        }
    };

    if hydrating_root || hydrating_child {
        let child_patches = initial_values
            .into_iter()
            .filter_map(|update| match update {
                Update::Child(patch) => Some(patch),
                _ => None,
            });
        let mut child_builders: Vec<ViewBuilder> = vec![];
        for patch in child_patches.into_iter() {
            let _ = child_builders.list_patch_apply(patch);
        }

        let node = dom
            .clone_as::<web_sys::Node>()
            .context("element is not a node")?;
        let mut children = dom.children.try_write().context("can't write children")?;
        for (i, bldr) in child_builders.into_iter().enumerate() {
            children.push(build(bldr, Some((i, &node)))?);
        }
    } else {
        for update in initial_values.into_iter() {
            dom.update(update)?;
        }
    }

    // add listeners
    for listener in listeners.into_iter() {
        dom.add_listener(listener)?;
    }

    // post build
    for op in post_build_ops.into_iter() {
        let mut any_view = AnyView::new(dom.clone());
        (op)(&mut any_view)?;
    }

    // make spawn update loop
    let mut to_spawn = vec![];
    if let Some(stream) = mogwai::stream::select_all(updates) {
        let (mut stream, handle) = stream_and_handle(stream);
        dom.update_handle = Some(handle);

        let node = dom.clone();
        to_spawn.push(FutureTask(Box::pin(async move {
            while let Some(update) = stream.next().await {
                node.update(update).unwrap_throw();
            }
        })));
    }

    // make spawn logic tasks
    for task in tasks.into_iter() {
        to_spawn.push(FutureTask(task));
    }

    // spawn them
    for future_task in to_spawn.into_iter() {
        spawn_local(future_task.0);
    }

    // send view sinks
    for sink in view_sinks.into_iter() {
        let any_view = AnyView::new(dom.clone());
        let _ = sink.try_send(any_view);
    }

    Ok(dom)
}

impl TryFrom<ViewBuilder> for JsDom {
    type Error = anyhow::Error;

    fn try_from(builder: ViewBuilder) -> Result<Self, Self::Error> {
        build(builder, None)
    }
}

/// Used to identify an existing node when hydrating `JsDom`.
pub enum HydrationKey {
    Id(String),
    IndexedChildOf { node: web_sys::Node, index: u32 },
}

impl HydrationKey {
    pub fn try_new(
        tag: impl AsRef<str>,
        attribs: impl Iterator<Item = HashPatch<String, String>>,
        may_parent: Option<(usize, &web_sys::Node)>,
    ) -> anyhow::Result<Self> {
        let mut attributes = HashMap::new();
        for patch in attribs.into_iter() {
            let _ = attributes.hash_patch_apply(patch);
        }

        if let Some(id) = attributes.remove("id") {
            return Ok(HydrationKey::Id(id));
        }

        if let Some((index, parent)) = may_parent {
            return Ok(HydrationKey::IndexedChildOf {
                node: parent.clone(),
                index: index as u32,
            });
        }

        anyhow::bail!(
            "Missing any hydration option for node '{}' - must be the child of a node or have an \
             id",
            tag.as_ref()
        )
    }

    pub fn hydrate(self) -> anyhow::Result<JsDom> {
        anyhow::ensure!(
            cfg!(target_arch = "wasm32"),
            "Hydration only available on WASM"
        );

        let el: web_sys::Node = match self {
            HydrationKey::Id(id) => {
                let el = crate::utils::document()
                    .clone_as::<web_sys::Document>()
                    .with_context(|| "wasm only")?
                    .get_element_by_id(&id)
                    .with_context(|| format!("Could not find an element with id '{}'", id))?;
                el.clone().dyn_into::<web_sys::Node>().map_err(|_| {
                    anyhow::anyhow!(
                        "Could not convert from '{}' to '{}' for value: {:#?}",
                        "Element",
                        "Node",
                        el,
                    )
                })?
            }
            HydrationKey::IndexedChildOf { node, index } => {
                let children = node.child_nodes();
                let mut non_empty_children = vec![];
                for i in 0..children.length() {
                    let child = children.get(i).with_context(|| {
                        format!(
                            "Child at index {} could not be found in node '{}' containing '{:?}'",
                            index,
                            node.node_name(),
                            node.node_value()
                        )
                    })?;
                    if child.node_type() == 3 {
                        // This is a text node
                        let has_text: bool = child
                            .node_value()
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| false);
                        if has_text {
                            non_empty_children.push(child);
                        }
                    } else {
                        non_empty_children.push(child);
                    }
                }
                let el = non_empty_children
                    .get(index as usize)
                    .with_context(|| {
                        format!(
                            "Child at index {} could not be found in non-empty children of node '{}' containing '{:?}'",
                            index,
                            node.node_name(),
                            node.node_value(),
                            //node,
                            //{
                            //    let mut nodes = vec![];
                            //    for i in 0..children.length() {
                            //        nodes.push(children.get(i).unwrap_throw().outer_html().unwrap_throw());
                            //    }
                            //    nodes
                            //}
                        )
                    })?
                    .clone();
                el
            }
        };

        Ok(JsDom::from_jscast(&el))
    }
}

/// Used to "hydrate" a `JsDom` from a ViewBuilder and pre-built DOM.
///
/// We use this when creating `JsDom` from DOM that was pre-rendered
/// server-side.
pub struct Hydrator {
    inner: JsDom,
}

impl From<Hydrator> for JsDom {
    fn from(Hydrator { inner }: Hydrator) -> Self {
        inner
    }
}

impl TryFrom<ViewBuilder> for Hydrator {
    type Error = anyhow::Error;

    /// Attempt to hydrate [`JsDom`] from [`ViewBuilder`].
    fn try_from(mut builder: ViewBuilder) -> anyhow::Result<Self> {
        if builder.hydration_root.is_none() {
            let attribs = builder
                .initial_values
                .iter()
                .filter_map(|update| match update {
                    Update::Attribute(patch) => Some(patch.clone()),
                    _ => None,
                });
            let key = match &builder.identity {
                ViewIdentity::Branch(t) => HydrationKey::try_new(t, attribs, None),
                ViewIdentity::NamespacedBranch(t, _) => HydrationKey::try_new(t, attribs, None),
                ViewIdentity::Leaf(t) => HydrationKey::try_new(t, attribs, None),
            }?;
            builder.hydration_root = Some(AnyView::new(key.hydrate()?));
        }

        let inner = build(builder, None)?;

        Ok(Hydrator { inner })
    }
}

//! Wrapper around Javascript DOM nodes.
use std::{
    collections::HashMap,
    future::Future,
    ops::{Bound, Deref, RangeBounds},
    pin::Pin,
    sync::{
        atomic::{self, AtomicUsize},
        Arc,
    },
};

use anyhow::Context;
use async_lock::{RwLock, RwLockUpgradableReadGuard};
use futures::{stream::select_all, FutureExt};
use mogwai::{
    channel::mpsc,
    sink::SinkExt,
    stream::StreamExt,
    patch::{HashPatch, HashPatchApply, ListPatch, ListPatchApply},
    view::{AnyEvent, Listener, Update, ViewBuilder, ViewIdentity},
};
use send_wrapper::SendWrapper;
use wasm_bindgen::{JsCast, JsValue};

use crate::event::JsDomEvent;

use super::FutureTask;

static NODE_ID: AtomicUsize = AtomicUsize::new(0);

pub struct JsTask<T: Send + 'static> {
    tx_cancel_task: Option<async_channel::Sender<()>>,
    inner: Arc<RwLock<Option<T>>>,
    // TODO: remove this debugging string
    name: String,
}

impl<T: Send + 'static> Clone for JsTask<T> {
    fn clone(&self) -> Self {
        Self {
            tx_cancel_task: self.tx_cancel_task.clone(),
            inner: self.inner.clone(),
            name: self.name.clone(),
        }
    }
}

impl<T: Send + 'static> Drop for JsTask<T> {
    fn drop(&mut self) {
        let _ = self.cancel();
        log::trace!("dropping JsTask '{}'", self.name);
    }
}

impl<T: Send + 'static> JsTask<T> {
    pub fn is_finished(&self) -> bool {
        self.inner
            .try_read()
            .map(|r| r.is_some())
            .unwrap_or_default()
    }

    pub async fn try_into_inner(self) -> Result<T, JsTask<T>> {
        let r = self.inner.upgradable_read().await;
        if r.is_some() {
            let mut w = RwLockUpgradableReadGuard::upgrade(r).await;
            Ok(w.take().unwrap())
        } else {
            drop(r);
            Err(self)
        }
    }

    /// Cancels the task, if possible. not yet finished.
    pub fn cancel(&mut self) -> anyhow::Result<()> {
        let cancel_tx = self.tx_cancel_task.take().context("already cancelled")?;
        let _ = cancel_tx.try_send(());
        log::trace!("cancelling JsTask '{}'", self.name);
        Ok(())
    }

    /// Detaches the task, running it in Javascript without the ability to be canceled.
    pub fn detach(mut self) {
        self.tx_cancel_task = None;
    }
}

/// Spawn an async task and return a `JsTask<T>`.
pub fn spawn_local<T: Send + 'static>(
    name: &str,
    future: impl Future<Output = T> + 'static,
) -> JsTask<T> {
    let inner = Arc::new(RwLock::new(None));
    let inner_spawned = inner.clone();
    let (tx_cancel_task, mut rx_cancel_task) = async_channel::bounded(1);
    wasm_bindgen_futures::spawn_local(async move {
        let task_done = async move {
            let t = future.await;
            let mut w = inner_spawned.write().await;
            *w = Some(t);
        }
        .into_stream()
        .boxed_local();
        let task_cancelled = async move {
            rx_cancel_task.next().await;
        }
        .into_stream()
        .boxed_local();
        select_all(vec![task_done, task_cancelled]).next().await;
    });
    JsTask {
        tx_cancel_task: Some(tx_cancel_task),
        inner,
        name: name.to_string(),
    }
}

pub(crate) fn init(_: &(), identity: ViewIdentity) -> anyhow::Result<JsDom> {
    let element = match identity {
        ViewIdentity::Branch(tag) => JsDom::element(&tag, None),
        ViewIdentity::NamespacedBranch(tag, ns) => JsDom::element(&tag, Some(&ns)),
        ViewIdentity::Leaf(text) => JsDom::text(&text),
    }?;
    Ok(element)
}

pub(crate) fn add_event(
    view: &JsDom,
    Listener {
        event_name,
        event_target,
        sink,
    }: Listener,
) -> anyhow::Result<FutureTask<()>> {
    let tx = sink.contra_map(|event: JsDomEvent| AnyEvent::new(event));
    let task = match event_target.as_str() {
        "myself" => crate::event::add_event(
            &view.name,
            &event_name,
            view.inner
                .dyn_ref::<web_sys::EventTarget>()
                .ok_or_else(|| "not an event target".to_string())
                .unwrap(),
            Box::pin(tx),
        ),
        "window" => crate::event::add_event(
            &view.name,
            &event_name,
            &web_sys::window().unwrap(),
            Box::pin(tx),
        ),
        "document" => crate::event::add_event(
            &view.name,
            &event_name,
            &web_sys::window().unwrap().document().unwrap(),
            Box::pin(tx),
        ),
        _ => anyhow::bail!("unsupported event target {}", event_target),
    };
    Ok(task)
}

/// A Javascript/browser DOM node.
///
/// Represents DOM nodes when a view is built on a WASM target.
#[derive(Clone)]
pub struct JsDom {
    pub(crate) name: Arc<String>,
    pub(crate) inner: SendWrapper<std::sync::Arc<JsValue>>,
    pub(crate) tasks: Arc<RwLock<Vec<JsTask<()>>>>,
    pub(crate) children: Arc<RwLock<Vec<JsDom>>>,
    pub(crate) parents_children: Option<Arc<RwLock<Vec<JsDom>>>>,
}

impl std::fmt::Display for JsDom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsDom").field("name", &self.name).finish()
    }
}

impl Drop for JsDom {
    fn drop(&mut self) {
        if Arc::strong_count(&self.children) == 1 {
            log::trace!(
                "dropping {}, which has {} refs",
                self,
                Arc::strong_count(&self.name)
            );
        }
    }
}

impl std::fmt::Debug for JsDom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsDom")
            .field("name", &self.name)
            .field("inner", &self.inner)
            .field(
                "tasks",
                &format!(
                    "vec(len={})",
                    self.tasks
                        .try_read()
                        .map(|vs| vs.len().to_string())
                        .unwrap_or("?".to_string())
                ),
            )
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
            name: Arc::new(format!("from {:?}", value)),
            inner: SendWrapper::new(std::sync::Arc::new(value)),
            tasks: Default::default(),
            children: Default::default(),
            parents_children: None,
        }
    }
}

pub(crate) fn update_js_dom(js_dom: &JsDom, update: Update) -> anyhow::Result<()> {
    match update {
        Update::Text(s) => {
            js_dom
                .inner
                .dyn_ref::<web_sys::Text>()
                .context("not a text node")?
                .set_data(&s);
        }
        Update::Attribute(patch) => match patch {
            HashPatch::Insert(k, v) => {
                js_dom
                    .inner
                    .dyn_ref::<web_sys::Element>()
                    .with_context(|| {
                        format!(
                            "could not set attribute {}={} on {:?}: not an element",
                            k, v, js_dom.inner
                        )
                    })?
                    .set_attribute(&k, &v)
                    .map_err(|_| anyhow::anyhow!("could not set attrib"))?;
            }
            HashPatch::Remove(k) => {
                js_dom
                    .inner
                    .dyn_ref::<web_sys::Element>()
                    .with_context(|| {
                        format!(
                            "could remove attribute {} on {:?}: not an element",
                            k, js_dom.inner
                        )
                    })?
                    .remove_attribute(&k)
                    .map_err(|_| anyhow::anyhow!("could remove attrib"))?;
            }
        },
        Update::BooleanAttribute(patch) => match patch {
            HashPatch::Insert(k, v) => {
                if v {
                    js_dom
                        .inner
                        .dyn_ref::<web_sys::Element>()
                        .with_context(|| {
                            format!(
                                "could not set boolean attribute {}={} on {:?}: not an element",
                                k, v, js_dom.inner
                            )
                        })?
                        .set_attribute(&k, "")
                        .map_err(|_| anyhow::anyhow!("could not set boolean attrib"))?;
                } else {
                    js_dom
                        .inner
                        .dyn_ref::<web_sys::Element>()
                        .with_context(|| {
                            format!(
                                "could not remove boolean attribute {}={} on {:?}: not an element",
                                k, v, js_dom.inner
                            )
                        })?
                        .remove_attribute(&k)
                        .map_err(|_| anyhow::anyhow!("could not remove boolean attrib"))?;
                }
            }
            HashPatch::Remove(k) => {
                js_dom
                    .inner
                    .dyn_ref::<web_sys::Element>()
                    .with_context(|| {
                        format!(
                            "could not remove boolean attribute {} on {:?}: not an element",
                            k, js_dom.inner
                        )
                    })?
                    .remove_attribute(&k)
                    .map_err(|_| anyhow::anyhow!("could not remove boolean attrib".to_string()))?;
            }
        },
        Update::Style(patch) => {
            let style = js_dom
                .inner
                .dyn_ref::<web_sys::HtmlElement>()
                .map(|el| el.style())
                .or_else(|| {
                    js_dom
                        .inner
                        .dyn_ref::<web_sys::SvgElement>()
                        .map(|el| el.style())
                })
                .with_context(|| {
                    format!(
                        "could not patch style on {:?}: not an element",
                        js_dom.inner
                    )
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
            let _ = js_dom.patch(patch);
        }
    }

    Ok(())
}

// TODO: Make errors returned by JsDom methods Box<dyn Error>
impl JsDom {
    pub fn name(&self) -> String {
        self.name.to_string()
    }

    /// Create a `JsDom` from anything that implements `JsCast`.
    pub fn from_jscast<T: JsCast>(t: &T) -> Self {
        let val = JsValue::from(t);
        JsDom::from(val)
    }

    /// Given a setter function on a `JsCast` type, return another setter function
    /// that takes `Self` and sets a value on it, if possible.
    ///
    /// If `Self` cannot be used as the given `JsCast` type, the returned setter function
    /// will log an error.
    ///
    /// This is useful in conjunction with the `capture:for_each` [`rsx`] macro attribute.
    /// See [`ViewBuilder::with_capture_for_each`] for more details.
    pub fn try_to<E: JsCast, S: ?Sized, T: AsRef<S>>(
        f: impl Fn(&E, &S) + Send + 'static,
    ) -> Box<dyn Fn(&Self, T) + Send + 'static> {
        Box::new(move |js: &JsDom, t: T| {
            let res = js.visit_as::<E, ()>(|el| f(el, t.as_ref()));
            if res.is_none() {
                log::error!("could not use {} as {}", js, std::any::type_name::<E>());
            }
        })
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
        let node_id = NODE_ID.fetch_add(1, atomic::Ordering::Relaxed);
        Ok(JsDom {
            name: Arc::new(format!("{}{}{}", tag, namespace.unwrap_or(""), node_id)),
            inner,
            tasks: Default::default(),
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
        let inner = SendWrapper::new(std::sync::Arc::new(node));
        let node_id = NODE_ID.fetch_add(1, atomic::Ordering::Relaxed);
        let len = s.char_indices().count();
        let tenth_ndx = s.char_indices().take(10).fold(0, |_, (ndx, _)| ndx);
        let ext = if len > 10 { "..." } else { "" };
        let trunc = &s[..tenth_ndx];
        Ok(JsDom {
            name: Arc::new(format!("'{}{}'{}", trunc, ext, node_id)),
            inner,
            tasks: Default::default(),
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

    /// Visits the inner node with a function, if the node can be cast correctly.
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
            .map(|js| js.clone_as::<web_sys::Node>().unwrap());

        log::trace!(
            "patching {} with {:?}",
            self,
            patch.as_ref().map(|js| format!("{}", js))
        );
        let mut parent = self.inner.dyn_ref::<web_sys::Node>().unwrap().clone();
        list_patch_apply_node(&mut parent, node_patch);

        let mut w = self.children.try_write().unwrap();
        let mut removed = w.list_patch_apply(patch.map(|mut js_dom| {
            js_dom.parents_children = Some(self.children.clone());
            js_dom
        }));
        for removed_child in removed.iter_mut() {
            removed_child.parents_children = None;
        }
        log::trace!("removed {} children from {}", removed.len(), self);
        removed
    }

    /// Conduct upkeep of this node, trimming any finished tasks
    fn upkeep<'a, 'b: 'a>(&'b self) -> Pin<Box<dyn Future<Output = usize> + 'a>> {
        Box::pin(async {
            let mut tasks = self.tasks.write().await;
            tasks.retain(|task| !task.is_finished());
            let mut total_retained_tasks = tasks.len();
            drop(tasks);

            let children = self.children.read().await;
            for child in children.iter() {
                let child_retained_tasks = child.upkeep().await;
                total_retained_tasks += child_retained_tasks;
            }
            total_retained_tasks
        })
    }

    /// Run this view in a parent container forever, never dropping it.
    pub fn run_in_container(self, container: JsDom) -> anyhow::Result<()> {
        log::info!("run in container");
        container.patch(ListPatch::push(self));
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                crate::core::time::wait_millis(10_000).await;
                let tasks = container.upkeep().await;
                log::info!("{} retained {} tasks after upkeep", container, tasks);
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
            let _ = tx.send(t).await.unwrap();
        });
        let t = rx.next().await.context("future never finished")?;
        Ok(t)
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

impl TryFrom<ViewBuilder> for JsDom {
    type Error = anyhow::Error;

    fn try_from(builder: ViewBuilder) -> Result<Self, Self::Error> {
        let (js, to_spawn) = super::build((), builder, |js| js.name.to_string(), init, update_js_dom, add_event)?;
        for future_task in to_spawn.into_iter() {
            log::trace!("spawning js task '{}'", future_task.name);
            let mut ts = js.tasks.try_write().unwrap();
            ts.push(spawn_local(&future_task.name, future_task.fut));
        }
        Ok(js)
    }
}

/// Used to identify an existing node when hydrating `JsDom`.
pub enum HydrationKey {
    Id(String),
    IndexedChildOf { node: web_sys::Node, index: u32 },
}

impl HydrationKey {
    pub fn try_new(
        tag: String,
        attribs: Vec<HashPatch<String, String>>,
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

        anyhow::bail!("Missing any hydration option for node '{}' - must be the child of a node or have an id", tag)
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
                            "Child at index {} could not be found in node '{}' containing '{:?}'",
                            index,
                            node.node_name(),
                            node.node_value()
                        )
                    })?
                    .clone();
                el
            }
        };

        //let dom = JsDom::from_jscast(&el);
        let dom = JsDom {
            name: Arc::new("hydrated".to_string()),
            inner: SendWrapper::new(Arc::new(JsValue::from(el))),
            tasks: Default::default(),
            children: Default::default(),
            parents_children: None,
        };
        Ok(dom)
    }
}

/// Used to "hydrate" a `JsDom` from a ViewBuilder and pre-built DOM.
///
/// We use this when creating `JsDom` from DOM that was pre-rendered server-side.
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

    fn try_from(builder: ViewBuilder) -> anyhow::Result<Self> {
        Hydrator::try_hydrate(builder, None)
    }
}

impl Hydrator {
    /// Attempt to hydrate [`JsDom`] from [`ViewBuilder`].
    fn try_hydrate(
        builder: ViewBuilder,
        may_parent: Option<(usize, &web_sys::Node)>,
    ) -> anyhow::Result<Hydrator> {
        let ViewBuilder {
            identity,
            updates,
            post_build_ops,
            view_sinks,
            listeners,
            tasks,
        } = builder;
        let construct_with = match identity {
            ViewIdentity::Branch(s) => s,
            ViewIdentity::NamespacedBranch(s, _) => s,
            ViewIdentity::Leaf(s) => s,
        };

        let (update_stream, updates) = crate::core::view::exhaust(select_all(updates));
        let (updates, attribs) =
            updates
                .into_iter()
                .fold((vec![], vec![]), |(mut updates, mut attribs), update| {
                    match update {
                        Update::Attribute(patch) => attribs.push(patch),
                        update => updates.push(update),
                    }
                    (updates, attribs)
                });

        let key = HydrationKey::try_new(construct_with, attribs, may_parent)?;
        let dom = key.hydrate()?;

        let (dom, tasks) = super::finalize_build(
            dom,
            |js| js.name.to_string(),
            update_stream,
            post_build_ops,
            listeners,
            tasks,
            view_sinks,
            add_event,
            update_js_dom,
        )?;

        let node = dom
            .clone_as::<web_sys::Node>()
            .context("element is not a node")?;
        let child_patches = updates.into_iter().filter_map(|update| match update {
            Update::Child(patch) => Some(patch),
            _ => None,
        });
        let mut children: Vec<ViewBuilder> = vec![];
        for patch in child_patches.into_iter() {
            let _ = children.list_patch_apply(patch);
        }

        for (bldr, i) in children.into_iter().zip(0..) {
            // we don't need to do anything with the hydrated JsDom because it is already
            // attached and its reactivity has been spawned
            let _ = Hydrator::try_hydrate(bldr, Some((i, &node)))?;
        }

        // lastly spawn all our tasks
        for fut_task in tasks.into_iter() {
            log::trace!("hydrator spawning task '{}'", fut_task.name);
            let mut ts = dom.tasks.try_write().unwrap();
            ts.push(spawn_local(&fut_task.name, fut_task.fut));
        }

        Ok(Hydrator { inner: dom })
    }
}

//! Provides string rendering for server-side mogwai nodes.
use anyhow::Context;
use async_executor::Executor;
use futures::{Future, Sink, StreamExt};
use async_lock::RwLock;
use std::{collections::HashMap, ops::DerefMut, pin::Pin, sync::Arc};

use mogwai::{
    builder::{ViewBuilder, ViewIdentity},
    channel::SinkError,
    futures::SinkExt,
    patch::{HashPatch, ListPatch, ListPatchApply},
    view::{EventTargetType, View, ViewResources},
};
use serde_json::Value;

// Only certain nodes can be "void" - which means written as <tag /> when
// the node contains no children. Writing non-void nodes in void notation
// does some spooky things to the DOM at parse-time.
//
// From https://riptutorial.com/html/example/4736/void-elements
// HTML 4.01/XHTML 1.0 Strict includes the following void elements:
//
//     rea - clickable, defined area in an image
//     base - specifies a base URL from which all links base
//     br - line break
//     col - column in a table [deprecated]
//     hr - horizontal rule (line)
//     img - image
//     input - field where users enter data
//     link - links an external resource to the document
//     meta - provides information about the document
//     param - defines parameters for plugins
//
//     HTML 5 standards include all non-deprecated tags from the previous list and
//
//     command - represents a command users can invoke [obsolete]
//     keygen - facilitates public key generation for web certificates [deprecated]
//     source - specifies media sources for picture, audio, and video elements
fn tag_is_voidable(tag: &str) -> bool {
    tag == "area"
        || tag == "base"
        || tag == "br"
        || tag == "col"
        || tag == "hr"
        || tag == "img"
        || tag == "input"
        || tag == "link"
        || tag == "meta"
        || tag == "param"
        || tag == "command"
        || tag == "keygen"
        || tag == "source"
}

/// DOM node variants.
pub enum SsrNode {
    /// Text node.
    Text(String),
    /// Parent node.
    Container {
        /// Tag name.
        name: String,
        /// Tag attributes.
        attributes: Vec<(String, Option<String>)>,
        /// Styles
        styles: Vec<(String, String)>,
        /// Child node list.
        children: Vec<SsrDom>,
    },
}

impl SsrNode {
    pub async fn html_string(&self) -> String {
        match self {
            SsrNode::Text(s) => s.to_string(),
            SsrNode::Container {
                name,
                attributes,
                children,
                styles,
            } => {
                let mut attributes = attributes.clone();
                if !styles.is_empty() {
                    let styles = styles
                        .iter()
                        .map(|(k, v)| format!("{}: {};", k, v))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let mut style_added = false;
                    for (key, value) in attributes.iter_mut() {
                        if key == "style" {
                            if let Some(prev_style) = value.as_mut() {
                                *prev_style = vec![prev_style.as_str(), styles.as_str()].join(" ");
                                style_added = true;
                                break;
                            }
                        }
                    }
                    if !style_added {
                        attributes.push(("style".into(), Some(styles)));
                    }
                }

                let atts = attributes
                    .iter()
                    .map(|(key, may_val)| {
                        if let Some(val) = may_val {
                            format!(r#"{}="{}""#, key, val)
                        } else {
                            format!("{}", key)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                if children.is_empty() {
                    if attributes.is_empty() {
                        if tag_is_voidable(&name) {
                            format!("<{} />", name)
                        } else {
                            format!("<{}></{}>", name, name)
                        }
                    } else {
                        if tag_is_voidable(&name) {
                            format!("<{} {} />", name, atts)
                        } else {
                            format!("<{} {}></{}>", name, atts, name)
                        }
                    }
                } else {
                    let kids: String = futures::stream::iter(children.into_iter())
                        .flat_map(|kid| futures::stream::once(kid.html_string()))
                        .map(|s: String| s.trim().to_string())
                        .collect::<Vec<String>>()
                        .await
                        .join(" ");
                    if attributes.is_empty() {
                        format!("<{}>{}</{}>", name, kids, name)
                    } else {
                        format!("<{} {}>{}</{}>", name, atts, kids, name)
                    }
                }
            }
        }
    }
}

/// A server side renderable DOM element.
#[derive(Clone)]
pub struct SsrDom {
    /// A shared executor for spawning and running futures.
    pub executor: Arc<Executor<'static>>,
    /// The underlying node.
    pub node: Arc<RwLock<SsrNode>>,
    /// A map of events registered with this element.
    pub events: Arc<
        RwLock<
            HashMap<
                (EventTargetType, String),
                Pin<Box<dyn Sink<Value, Error = SinkError> + Send + Sync + 'static>>,
            >,
        >,
    >,
}

impl TryFrom<ViewBuilder<SsrDom>> for SsrDom {
    type Error = anyhow::Error;

    fn try_from(value: ViewBuilder<SsrDom>) -> Result<Self, Self::Error> {
        let executor = Arc::new(Executor::default());
        SsrDomResources(executor).build(value)
    }
}

#[cfg(test)]
mod ssr {
    #[test]
    fn ssrelement_sendable() {
        fn sendable<T: Send + Sync + 'static>() {}
        sendable::<super::SsrDom>()
    }
}

impl SsrDom {
    /// Creates a text node.
    pub fn text(executor: Arc<Executor<'static>>, s: &str) -> Self {
        SsrDom {
            executor,
            node: Arc::new(RwLock::new(SsrNode::Text(
                s.replace("&", "&amp;")
                    .replace("<", "&lt;")
                    .replace(">", "&gt;")
                    .into(),
            ))),
            events: Default::default(),
        }
    }

    /// Creates a container node that may contain child nodes.
    pub fn element(executor: Arc<Executor<'static>>, tag: &str) -> Self {
        SsrDom {
            executor,
            node: Arc::new(RwLock::new(SsrNode::Container {
                name: tag.into(),
                attributes: vec![],
                styles: vec![],
                children: vec![],
            })),
            events: Default::default(),
        }
    }

    /// Set the text.
    ///
    /// Fails if this element is not a text node.
    pub fn set_text(&self, text: &str) -> anyhow::Result<()> {
        let mut lock = self.node.try_write().context("can't lock for writing")?;
        if let SsrNode::Text(prev) = lock.deref_mut() {
            *prev = text
                .replace("&", "&amp;")
                .replace("<", "&lt;")
                .replace(">", "&gt;")
                .to_string();
        } else {
            anyhow::bail!("not a text node");
        }
        Ok(())
    }

    /// Add an attribute.
    ///
    /// Fails if this element is not a container.
    pub fn set_attrib(&self, key: &str, value: Option<&str>) -> anyhow::Result<()> {
        let mut lock = self.node.try_write().context("can't lock for writing")?;
        if let SsrNode::Container { attributes, .. } = lock.deref_mut() {
            for (pkey, pval) in attributes.iter_mut() {
                if pkey == &key {
                    *pval = value.map(String::from);
                    return Ok(());
                }
            }

            attributes.push((key.to_string(), value.map(|v| v.to_string())));
        } else {
            anyhow::bail!("not a container node");
        }
        Ok(())
    }

    /// Get an attribute
    pub fn get_attrib(&self, key: &str) -> anyhow::Result<Option<String>> {
        let mut lock = self.node.try_write().context("can't lock for writing")?;
        if let SsrNode::Container { attributes, .. } = lock.deref_mut() {
            for (pkey, pval) in attributes.iter() {
                if pkey == &key {
                    return Ok(pval.as_ref().cloned());
                }
            }
            anyhow::bail!("no such attribute")
        } else {
            anyhow::bail!("not an element")
        }
    }

    /// Remove an attribute.
    ///
    /// Fails if this is not a container element.
    pub fn remove_attrib(&self, key: &str) -> anyhow::Result<()> {
        let mut lock = self.node.try_write().context("can't lock for writing")?;
        if let SsrNode::Container { attributes, .. } = lock.deref_mut() {
            attributes.retain(|p| p.0 != key);
        } else {
            anyhow::bail!("not a container")
        }
        Ok(())
    }

    /// Add a style property.
    ///
    /// Fails if this is not a container element.
    pub fn set_style(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let mut lock = self.node.try_write().context("can't lock for writing")?;
        if let SsrNode::Container { styles, .. } = lock.deref_mut() {
            for (pkey, pval) in styles.iter_mut() {
                if pkey == &key {
                    *pval = value.to_string();
                    return Ok(());
                }
            }

            styles.push((key.to_string(), value.to_string()));
        } else {
            anyhow::bail!("not a container")
        }
        Ok(())
    }

    /// Remove a style property.
    ///
    /// Fails if this not a container element.
    pub fn remove_style(&self, key: &str) -> anyhow::Result<()> {
        let mut lock = self.node.try_write().context("can't lock for writing")?;
        if let SsrNode::Container { styles, .. } = lock.deref_mut() {
            styles.retain(|p| p.0 != key);
        } else {
            anyhow::bail!("not a container")
        }
        Ok(())
    }

    /// Fires an event downstream to any listening [`Stream`][mogwai_core::futures::Stream]s.
    ///
    /// Fails if no such event exists or if sending to the sink encounters an error.
    pub async fn fire_event(
        &self,
        type_is: EventTargetType,
        name: String,
        event: Value,
    ) -> Result<(), futures::future::Either<(), SinkError>> {
        use futures::future::Either;
        let mut events = self.events.write().await;
        let sink = events
            .deref_mut()
            .get_mut(&(type_is, name))
            .ok_or(Either::Left(()))?;
        sink.send(event).await.map_err(Either::Right)
    }

    /// Removes an event.
    pub fn remove_event(&self, type_is: EventTargetType, name: &str) {
        let mut lock = self.events.try_write().unwrap();
        let _ = lock.remove(&(type_is, name.to_string()));
    }

    /// String value
    pub fn html_string(&self) -> Pin<Box<dyn Future<Output = String> + Send>> {
        let node = self.node.clone();
        Box::pin(async move {
            let lock = node.read().await;
            lock.html_string().await
        })
    }

    pub fn run_while<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> anyhow::Result<T> {
        let (mut tx, mut rx) = mogwai::channel::mpsc::bounded::<T>(1);
        let send = async move {
            let t = fut.await;
            tx.send(t).await.unwrap();
        };
        let _task = self.executor.spawn(send);
        loop {
            match rx.inner.try_next() {
                Err(_) => {}
                Ok(may_t) => {
                    return may_t.ok_or_else(|| anyhow::anyhow!("future cannot finish"));
                }
            }
        }
    }
}

/// A wrapper around `async_executor::Executor`, which is needed to create
/// an `SsrDom`.
#[derive(Default)]
pub struct SsrDomResources(Arc<Executor<'static>>);

impl ViewResources<SsrDom> for SsrDomResources {
    fn init(&self, identity: ViewIdentity) -> anyhow::Result<SsrDom> {
        Ok(match identity {
            ViewIdentity::Branch(tag) => SsrDom::element(self.0.clone(), &tag),
            ViewIdentity::NamespacedBranch(tag, ns) => {
                let el = SsrDom::element(self.0.clone(), &tag);
                el.set_attrib("xmlns", Some(&ns))?;
                el
            }
            ViewIdentity::Leaf(text) => SsrDom::text(self.0.clone(), &text),
        })
    }
}

impl View for SsrDom {
    type Event = Value;
    type Child = Self;
    type Resources = SsrDomResources;

    /// Possibly asynchronous and scoped acquisition of resources.
    ///
    /// Used to build children before patching.
    fn with_acquired_resources<'a, T: Send + Sync + 'static>(
        &self,
        f: impl FnOnce(Self::Resources) -> anyhow::Result<T> + Send + Sync + 'a,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + Sync + 'a>> {
        let rez = SsrDomResources(self.executor.clone());
        Box::pin(async move { f(rez) })
    }

    /// Set the text of this view.
    fn set_text(&self, s: &str) -> anyhow::Result<()> {
        self.set_text(s)
    }

    /// Patch the attributes of this view.
    fn patch_attribs(&self, patch: HashPatch<String, String>) -> anyhow::Result<()> {
        match patch {
            HashPatch::Insert(k, v) => self.set_attrib(&k, Some(&v)),
            HashPatch::Remove(k) => self.remove_attrib(&k),
        }
    }

    /// Patch the boolean attributes of this view.
    fn patch_bool_attribs(&self, patch: HashPatch<String, bool>) -> anyhow::Result<()> {
        match patch {
            HashPatch::Insert(k, v) => {
                if v {
                    self.set_attrib(&k, None)
                } else {
                    self.remove_attrib(&k)
                }
            }
            HashPatch::Remove(k) => self.remove_attrib(&k),
        }
    }

    /// Patch the style attributes of this view.
    fn patch_styles(&self, patch: HashPatch<String, String>) -> anyhow::Result<()> {
        match patch {
            HashPatch::Insert(k, v) => self.set_style(&k, &v),
            HashPatch::Remove(k) => self.remove_style(&k),
        }
    }

    /// Patch the nested children of this view.
    ///
    /// Returns a vector of the children removed.
    fn patch_children(&self, patch: ListPatch<Self::Child>) -> anyhow::Result<Vec<Self::Child>> {
        let mut lock = self.node.try_write().context("can't lock")?;
        if let SsrNode::Container { children, .. } = lock.deref_mut() {
            Ok(children.list_patch_apply(patch))
        } else {
            anyhow::bail!("not a container")
        }
    }

    /// Add an event to the element, document or window.
    ///
    /// When an event occurs it will be sent into the given sink.
    fn set_event(
        &self,
        type_is: EventTargetType,
        name: &str,
        sink: impl Sink<Self::Event, Error = SinkError> + Unpin + Send + Sync + 'static,
    ) -> anyhow::Result<()> {
        let mut lock = self.events.try_write().context("can't lock")?;
        let _ = lock.insert((type_is, name.to_string()), Box::pin(sink));
        Ok(())
    }

    ///// Spawn an asynchronous task.
    fn spawn(
        &self,
        action: impl Future<Output = ()> + Send + 'static,
    ) {
        self.executor.spawn(action).detach()
    }
}

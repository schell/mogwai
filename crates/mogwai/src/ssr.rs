//! Provides string rendering for server-side mogwai nodes.

use futures::{lock::Mutex, Sink, SinkExt};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
};

use crate::{
    builder::EventTargetType,
    channel::SinkError,
    patch::{ListPatch, ListPatchApply},
};

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
pub enum SsrNode<Event> {
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
        children: Vec<SsrElement<Event>>,
    },
}

impl<Event> From<&SsrNode<Event>> for String {
    fn from(node: &SsrNode<Event>) -> String {
        match node {
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
                    let kids = children
                        .into_iter()
                        .map(|k| {
                            futures::executor::block_on(async {
                                let node = k.node.lock().await;
                                String::from(node.deref()).trim().to_string()
                            })
                        })
                        .collect::<Vec<String>>()
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

/// A server side renderable view element.
pub struct SsrElement<Event> {
    /// The underlying node.
    pub node: Arc<Mutex<SsrNode<Event>>>,
    /// A map of events registered with this element.
    pub events: Arc<
        Mutex<HashMap<(EventTargetType, String), Pin<Box<dyn Sink<Event, Error = SinkError>>>>>,
    >,
}

impl<Event> Clone for SsrElement<Event> {
    fn clone(&self) -> Self {
        SsrElement {
            node: self.node.clone(),
            events: self.events.clone(),
        }
    }
}

impl<Event> SsrElement<Event> {
    /// Creates a text node.
    pub fn text(s: &str) -> Self {
        SsrElement {
            node: Arc::new(Mutex::new(SsrNode::Text(s.into()))),
            events: Default::default(),
        }
    }

    /// Creates a container node that may contain child nodes.
    pub fn element(tag: &str) -> Self {
        SsrElement {
            node: Arc::new(Mutex::new(SsrNode::Container {
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
    pub async fn with_text(self, text: String) -> Result<Self, ()> {
        let mut lock = self.node.lock().await;
        if let SsrNode::Text(prev) = lock.deref_mut() {
            *prev = text;
        } else {
            return Err(());
        }
        drop(lock);
        Ok(self)
    }

    /// Add an attribute.
    ///
    /// Fails if this element is not a container.
    pub async fn with_attrib(self, key: String, value: Option<String>) -> Result<Self, ()> {
        let mut lock = self.node.lock().await;
        if let SsrNode::Container { attributes, .. } = lock.deref_mut() {
            let mut index = None;
            for ((pkey, _), i) in attributes.iter().zip(0..) {
                if pkey == &key {
                    index = Some(i);
                    break;
                }
            }

            let index = index.unwrap_or(0);
            attributes.insert(index, (key, value))
        } else {
            return Err(());
        }
        drop(lock);
        Ok(self)
    }

    /// Remove an attribute.
    ///
    /// Fails if this is not a container element.
    pub async fn without_attrib(self, key: String) -> Result<Self, ()> {
        let mut lock = self.node.lock().await;
        if let SsrNode::Container { attributes, .. } = lock.deref_mut() {
            attributes.retain(|p| p.0 != key);
        } else {
            return Err(());
        }
        drop(lock);
        Ok(self)
    }

    /// Add a style property.
    ///
    /// Fails if this is not a container element.
    pub async fn with_style(self, key: String, value: String) -> Result<Self, ()> {
        let mut lock = self.node.lock().await;
        if let SsrNode::Container { styles, .. } = lock.deref_mut() {
            let mut index = None;
            for ((pkey, _), i) in styles.iter().zip(0..) {
                if pkey == &key {
                    index = Some(i);
                    break;
                }
            }

            let index = index.unwrap_or(0);
            styles.insert(index, (key, value));
        } else {
            return Err(());
        }
        drop(lock);
        Ok(self)
    }

    /// Remove a style property.
    ///
    /// Fails if this not a container element.
    pub async fn without_style(self, key: String) -> Result<Self, ()> {
        let mut lock = self.node.lock().await;
        if let SsrNode::Container { styles, .. } = lock.deref_mut() {
            styles.retain(|p| p.0 != key);
        } else {
            return Err(());
        }
        drop(lock);
        Ok(self)
    }

    /// Add an event.
    ///
    /// Does not fail. `Ok` is returned to simplify the API.
    pub async fn with_event(
        self,
        type_is: EventTargetType,
        name: String,
        tx: Pin<Box<dyn Sink<Event, Error = SinkError>>>,
    ) -> Result<Self, ()> {
        let mut lock = self.events.lock().await;
        let _ = lock.insert((type_is, name), tx);
        drop(lock);
        Ok(self)
    }

    /// Fires an event downstream to any listening [`Stream`]s.
    ///
    /// Fails if no such event exists or if sending to the sink encounters an error.
    pub async fn fire_event(
        &self,
        type_is: EventTargetType,
        name: String,
        event: Event,
    ) -> Result<(), futures::future::Either<(), SinkError>> {
        use futures::future::Either;
        let mut events = self.events.lock().await;
        let sink = events.get_mut(&(type_is, name)).ok_or(Either::Left(()))?;
        sink.send(event).await.map_err(Either::Right)
    }

    /// Removes an event.
    ///
    /// Does not fail. `Ok` is returned to simplify the API.
    pub async fn without_event(self, type_is: EventTargetType, name: String) -> Result<Self, ()> {
        let mut lock = self.events.lock().await;
        let _ = lock.remove(&(type_is, name));
        drop(lock);
        Ok(self)
    }

    /// Patches child nodes.
    ///
    /// Fails if this is not a container element.
    pub async fn with_patch_children(self, patch: ListPatch<Self>) -> Result<Self, ()> {
        let mut lock = self.node.lock().await;
        if let SsrNode::Container { children, .. } = lock.deref_mut() {
            let _ = children.list_patch_apply(patch);
        } else {
            return Err(());
        }
        drop(lock);
        Ok(self)
    }
}

//! Provides string rendering for server-side mogwai nodes.

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    builder::EventTargetType,
    futures::{SinkExt, SinkError},
    patch::{ListPatch, ListPatchApply},
    spawn::Sinking,
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
                            let node = k.node.lock().unwrap();
                            String::from(node.deref()).trim().to_string()
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
    pub events: Arc<Mutex<HashMap<(EventTargetType, String), Pin<Box<Sinking<Event>>>>>>,
}

#[cfg(test)]
mod ssr {
    #[test]
    fn ssrelement_sendable() {
        fn sendable<T: crate::spawn::Sendable>() {}
        sendable::<super::SsrElement<web_sys::Event>>()
    }
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
            node: Arc::new(Mutex::new(SsrNode::Text(
                s.replace("&", "&amp;")
                    .replace("<", "&lt;")
                    .replace(">", "&gt;")
                    .into(),
            ))),
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
    pub fn set_text(&self, text: &str) -> Result<(), ()> {
        let mut lock = self.node.lock().unwrap();
        if let SsrNode::Text(prev) = lock.deref_mut() {
            *prev = text
                .replace("&", "&amp;")
                .replace("<", "&lt;")
                .replace(">", "&gt;")
                .to_string();
        } else {
            return Err(());
        }
        Ok(())
    }

    /// Add an attribute.
    ///
    /// Fails if this element is not a container.
    pub fn set_attrib(&self, key: &str, value: Option<&str>) -> Result<(), ()> {
        let mut lock = self.node.lock().unwrap();
        if let SsrNode::Container { attributes, .. } = lock.deref_mut() {
            for (pkey, pval) in attributes.iter_mut() {
                if pkey == &key {
                    *pval = value.map(String::from);
                    return Ok(());
                }
            }

            attributes.push((key.to_string(), value.map(|v| v.to_string())));
        } else {
            return Err(());
        }
        Ok(())
    }

    /// Remove an attribute.
    ///
    /// Fails if this is not a container element.
    pub fn remove_attrib(&self, key: &str) -> Result<(), ()> {
        let mut lock = self.node.lock().unwrap();
        if let SsrNode::Container { attributes, .. } = lock.deref_mut() {
            attributes.retain(|p| p.0 != key);
        } else {
            return Err(());
        }
        Ok(())
    }

    /// Add a style property.
    ///
    /// Fails if this is not a container element.
    pub fn set_style(&self, key: &str, value: &str) -> Result<(), ()> {
        let mut lock = self.node.lock().unwrap();
        if let SsrNode::Container { styles, .. } = lock.deref_mut() {
            for (pkey, pval) in styles.iter_mut() {
                if pkey == &key {
                    *pval = value.to_string();
                    return Ok(());
                }
            }

            styles.push((key.to_string(), value.to_string()));
        } else {
            return Err(());
        }
        Ok(())
    }

    /// Remove a style property.
    ///
    /// Fails if this not a container element.
    pub fn remove_style(&self, key: &str) -> Result<(), ()> {
        let mut lock = self.node.lock().unwrap();
        if let SsrNode::Container { styles, .. } = lock.deref_mut() {
            styles.retain(|p| p.0 != key);
        } else {
            return Err(());
        }
        Ok(())
    }

    /// Add an event.
    pub fn set_event(&self, type_is: EventTargetType, name: &str, tx: Pin<Box<Sinking<Event>>>) {
        let mut lock = self.events.lock().unwrap();
        let _ = lock.insert((type_is, name.to_string()), tx);
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
        let mut events = self.events.lock().unwrap();
        let sink = events
            .deref_mut()
            .get_mut(&(type_is, name))
            .ok_or(Either::Left(()))?;
        sink.send(event).await.map_err(Either::Right)
    }

    /// Removes an event.
    pub fn remove_event(&self, type_is: EventTargetType, name: &str) {
        let mut lock = self.events.lock().unwrap();
        let _ = lock.remove(&(type_is, name.to_string()));
    }

    /// Patches child nodes.
    ///
    /// Fails if this is not a container element.
    pub fn patch_children(&self, patch: ListPatch<Self>) -> Result<(), ()> {
        let mut lock = self.node.lock().unwrap();
        if let SsrNode::Container { children, .. } = lock.deref_mut() {
            let _ = children.list_patch_apply(patch);
        } else {
            return Err(());
        }
        Ok(())
    }

    /// String value
    pub fn html_string(&self) -> String {
        let lock = self.node.lock().unwrap();
        String::from(lock.deref())
    }
}

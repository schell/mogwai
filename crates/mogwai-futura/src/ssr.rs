//! Utilities for creating server-side rendered views.

use crate::{Str, sync::Shared};

pub struct EventListener {
    tx: async_channel::Sender<()>,
    rx: async_channel::Receiver<()>,
}

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
//     HTML 5 standards include all non-deprecated tags from the previous list
// and
//
//     command - represents a command users can invoke [obsolete]
//     keygen - facilitates public key generation for web certificates
// [deprecated]     source - specifies media sources for picture, audio, and
// video elements
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

/// Text node.
#[derive(Clone, Default)]
pub struct Text(Shared<Str>);

impl Text {
    pub fn new(text: impl Into<Str>) -> Self {
        Self(Shared::new(text.into()))
    }

    pub fn html_string(&self) -> String {
        self.0.get().to_string()
    }

    /// Set the text.
    pub fn set_text(&self, text: impl Into<Str>) {
        self.0.set(text.into());
    }

    /// Get the text.
    pub fn get_text(&self) -> Str {
        self.0.get().clone()
    }
}

/// Container node.
#[derive(Clone, Default)]
pub struct Container {
    /// Tag name.
    name: Str,
    /// Tag attributes.
    attributes: Shared<Vec<(Str, Option<Str>)>>,
    /// Styles
    styles: Shared<Vec<(Str, Str)>>,
    /// Child node list.
    children: Shared<Vec<Node>>,
}

impl Container {
    pub fn new(name: impl Into<Str>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Add an attribute.
    pub fn set_property(&self, key: impl Into<Str>, value: impl Into<Str>) {
        let mut attributes = self.attributes.get_mut();
        let (k, v) = (key.into(), value.into());
        for (k_prev, v_prev) in attributes.iter_mut() {
            if k_prev.as_str() == k.as_str() {
                *v_prev = Some(v);
                return;
            }
        }
        attributes.push((k, Some(v)));
    }

    /// Get the value of an attribute.
    pub fn has_property(&self, key: impl AsRef<str>) -> bool {
        for (pkey, _pval) in self.attributes.get().iter() {
            if pkey.as_str() == key.as_ref() {
                return true;
            }
        }
        false
    }

    /// Get the value of an attribute.
    pub fn get_property(&self, key: impl AsRef<str>) -> Option<Str> {
        for (pkey, pval) in self.attributes.get().iter() {
            if pkey.as_str() == key.as_ref() {
                return pval.clone();
            }
        }
        None
    }

    /// Remove an attribute.
    ///
    /// Returns the previous value, if any.
    pub fn remove_attrib(&self, key: impl AsRef<str>) -> Option<Str> {
        let mut value = None;
        self.attributes.get_mut().retain_mut(|p| {
            if p.0.as_str() == key.as_ref() {
                value = p.1.take();
                false
            } else {
                true
            }
        });
        value
    }

    /// Add a style property.
    pub fn set_style(&self, key: impl Into<Str>, value: impl Into<Str>) {
        let mut styles = self.styles.get_mut();
        let key = key.into();
        for (pkey, pval) in styles.iter_mut() {
            if pkey.as_str() == key.as_str() {
                *pval = value.into();
                return;
            }
        }
        styles.push((key, value.into()));
    }

    /// Remove a style property.
    ///
    /// Returns the previous style value, if any.
    pub fn remove_style(&self, key: impl AsRef<str>) -> Option<Str> {
        let mut value = None;
        self.styles.get_mut().retain_mut(|p| {
            if p.0.as_str() == key.as_ref() {
                value = Some(std::mem::replace(&mut p.1, "".into()));
                false
            } else {
                true
            }
        });
        value
    }

    /// Add a child.
    pub fn append_child(&self, child: impl Into<Node>) {
        self.children.get_mut().push(child.into());
    }

    pub fn html_string(&self) -> String {
        let name = &self.name;
        let styles = self.styles.get();
        let attributes = self.attributes.get_mut();
        let children = self.children.get();

        let mut attributes = attributes.clone();
        if !styles.is_empty() {
            let styles = styles
                .iter()
                .map(|(k, v)| format!("{}: {};", k, v))
                .collect::<Vec<_>>()
                .join(" ");

            let mut style_added = false;
            for (key, value) in attributes.iter_mut() {
                if key.as_str() == "style" {
                    if let Some(prev_style) = value.as_mut() {
                        *prev_style = [prev_style.as_str(), styles.as_str()].join(" ").into();
                        style_added = true;
                        break;
                    }
                }
            }
            if !style_added {
                attributes.push(("style".into(), Some(styles.into())));
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
                if tag_is_voidable(name.as_str()) {
                    format!("<{} />", name)
                } else {
                    format!("<{}></{}>", name, name)
                }
            } else if tag_is_voidable(name.as_str()) {
                format!("<{} {} />", name, atts)
            } else {
                format!("<{} {}></{}>", name, atts, name)
            }
        } else {
            let mut kids = vec![];
            for kid in children.iter() {
                let node = kid.html_string();
                kids.push(node);
            }
            let kids: String = kids.join(" ");
            if attributes.is_empty() {
                format!("<{}>{}</{}>", name, kids, name)
            } else {
                format!("<{} {}>{}</{}>", name, atts, kids, name)
            }
        }
    }
}

/// Child node.
#[derive(Clone)]
pub enum Node {
    Text(Text),
    Container(Container),
}

impl From<Text> for Node {
    fn from(value: Text) -> Self {
        Node::Text(value)
    }
}

impl From<Container> for Node {
    fn from(value: Container) -> Self {
        Node::Container(value)
    }
}

impl Node {
    pub fn html_string(&self) -> String {
        match self {
            Node::Text(text) => text.html_string(),
            Node::Container(container) => container.html_string(),
        }
    }

    /// Creates a text node.
    pub fn text(text: impl Into<Str>) -> Self {
        let text = text.into();
        Node::Text(Text(text.into()))
    }

    /// Creates a container node that may contain child nodes.
    pub fn element(tag: impl Into<Str>) -> Self {
        Node::Container(Container {
            name: tag.into(),
            attributes: Default::default(),
            styles: Default::default(),
            children: Default::default(),
        })
    }
}

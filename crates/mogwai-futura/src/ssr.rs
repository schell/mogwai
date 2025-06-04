//! Server-side rendered views.
use std::borrow::Cow;

use crate::{Str, sync::Shared, view::*};

pub mod prelude {
    pub use super::{Ssr, SsrElement, SsrEventListener, SsrEventTarget, SsrText};
    pub use crate::prelude::*;
}

#[derive(Clone)]
pub struct SsrText {
    pub text: Shared<Str>,
    pub events: Shared<Vec<SsrEventListener>>,
}

impl PartialEq for SsrText {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl ViewText for SsrText {
    fn new(text: impl AsRef<str>) -> Self {
        SsrText {
            text: Shared::from_str(text),
            events: Default::default(),
        }
    }

    fn set_text(&self, text: impl AsRef<str>) {
        let cow = Cow::from(text.as_ref().to_owned());
        self.text.set(cow);
    }

    fn get_text(&self) -> Str {
        self.text.get().clone()
    }
}

impl ViewChild<Ssr> for SsrText {
    fn as_append_arg(&self) -> AppendArg<Ssr, impl Iterator<Item = Cow<'_, SsrNode>>> {
        AppendArg::new(std::iter::once(Cow::Owned(SsrNode::Text(self.clone()))))
    }
}

impl ViewEventTarget<Ssr> for SsrText {
    fn listen(&self, event_name: impl Into<Str>) -> <Ssr as View>::EventListener {
        let listener = SsrEventListener::new(SsrEventTarget::Node(self.clone().into()), event_name);
        self.events.get_mut().push(listener.clone());
        listener
    }
}

/// Builder for runtime views.
#[derive(Clone)]
pub struct SsrElement {
    pub name: Str,
    pub attributes: Shared<Vec<(Str, Option<Str>)>>,
    pub styles: Shared<Vec<(Str, Str)>>,
    pub events: Shared<Vec<SsrEventListener>>,
    pub children: Shared<Vec<SsrNode>>,
}

impl PartialEq for SsrElement {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.attributes == other.attributes
            && self.styles == other.styles
            && self.events == other.events
            && self.children == other.children
    }
}

impl ViewParent<Ssr> for SsrElement {
    fn new(name: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().to_owned().into(),
            attributes: Default::default(),
            styles: Default::default(),
            events: Default::default(),
            children: Default::default(),
        }
    }

    fn new_namespace(name: impl AsRef<str>, ns: impl AsRef<str>) -> Self {
        let s = <SsrElement as ViewParent<Ssr>>::new(name);
        s.set_property("xmlns", ns);
        s
    }

    fn append_node(&self, node: Cow<'_, <Ssr as View>::Node>) {
        self.children.get_mut().push(node.into_owned());
    }

    fn remove_node(&self, node: Cow<'_, <Ssr as View>::Node>) {
        self.children
            .get_mut()
            .retain(|child| child != node.as_ref());
    }

    fn replace_node(
        &self,
        new_node: Cow<'_, <Ssr as View>::Node>,
        old_node: Cow<'_, <Ssr as View>::Node>,
    ) {
        let mut children = self.children.get_mut();
        let found = children
            .iter_mut()
            .find(|child| *child == old_node.as_ref());
        if let Some(node) = found {
            *node = new_node.into_owned();
        }
    }

    fn insert_node_before(
        &self,
        new_node: Cow<'_, <Ssr as View>::Node>,
        before_node: Option<Cow<'_, <Ssr as View>::Node>>,
    ) {
        if let Some(before_node) = before_node {
            let mut children = self.children.get_mut();
            let found = children.iter().enumerate().find_map(|(i, child)| {
                if child == before_node.as_ref() {
                    Some(i)
                } else {
                    None
                }
            });
            if let Some(index) = found {
                children.insert(index, new_node.into_owned());
            }
        } else {
            self.append_node(new_node);
        }
    }
}

impl ViewChild<Ssr> for SsrElement {
    fn as_append_arg(&self) -> AppendArg<Ssr, impl Iterator<Item = Cow<'_, SsrNode>>> {
        AppendArg::new(std::iter::once(Cow::Owned(SsrNode::Element(self.clone()))))
    }
}

impl ViewProperties for SsrElement {
    fn set_property(&self, key: impl AsRef<str>, value: impl AsRef<str>) {
        let mut attributes = self.attributes.get_mut();
        let (k, v) = (
            key.as_ref().to_owned().into(),
            value.as_ref().to_owned().into(),
        );
        for (k_prev, v_prev) in attributes.iter_mut() {
            if k_prev == &k {
                *v_prev = Some(v);
                return;
            }
        }
        attributes.push((k, Some(v)));
    }

    fn has_property(&self, key: impl AsRef<str>) -> bool {
        for (pkey, _pval) in self.attributes.get().iter() {
            if pkey == key.as_ref() {
                return true;
            }
        }
        false
    }

    fn get_property(&self, key: impl AsRef<str>) -> Option<Str> {
        for (pkey, pval) in self.attributes.get().iter() {
            if pkey == key.as_ref() {
                return pval.clone();
            }
        }
        None
    }

    fn remove_property(&self, key: impl AsRef<str>) {
        self.attributes
            .get_mut()
            .retain_mut(|p| p.0 != key.as_ref());
    }

    /// Add a style property.
    fn set_style(&self, key: impl AsRef<str>, value: impl AsRef<str>) {
        let mut styles = self.styles.get_mut();
        let key = key.as_ref().to_owned().into();
        let value = value.as_ref().to_owned().into();
        for (pkey, pval) in styles.iter_mut() {
            if pkey == &key {
                *pval = value;
                return;
            }
        }
        styles.push((key, value));
    }

    /// Remove a style property.
    ///
    /// Returns the previous style value, if any.
    fn remove_style(&self, key: impl AsRef<str>) {
        self.styles.get_mut().retain_mut(|p| p.0 != key.as_ref());
    }
}

impl ViewEventTarget<Ssr> for SsrElement {
    fn listen(&self, event_name: impl Into<Str>) -> SsrEventListener {
        let event_listener = SsrEventListener::new(
            SsrEventTarget::Node(SsrNode::Element(self.clone())),
            event_name,
        );
        self.events.get_mut().push(event_listener.clone());
        event_listener
    }
}

impl SsrElement {
    pub fn html_string(&self) -> String {
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
                if key == "style" {
                    if let Some(prev_style) = value.take() {
                        let spaced = (prev_style + " " + styles.as_str()).into_owned();
                        *value = Some(spaced.into());
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
                if tag_is_voidable(name) {
                    format!("<{} />", name)
                } else {
                    format!("<{}></{}>", name, name)
                }
            } else if tag_is_voidable(name) {
                format!("<{} {} />", name, atts)
            } else {
                format!("<{} {}></{}>", name, atts, name)
            }
        } else {
            let mut kids = vec![];
            for kid in children.iter() {
                let node = match kid {
                    SsrNode::Element(element_builder) => element_builder.html_string(),
                    SsrNode::Text(text_builder) => text_builder.text.get().to_string(),
                };
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

#[derive(Clone, PartialEq)]
pub enum SsrNode {
    Element(SsrElement),
    Text(SsrText),
}

// impl ViewNode for SsrNode {
//     type Owned = Self;
//     fn owned_node(self) -> Self {
//         self
//     }
// }

impl From<SsrText> for SsrNode {
    fn from(value: SsrText) -> Self {
        SsrNode::Text(value)
    }
}

impl From<SsrElement> for SsrNode {
    fn from(value: SsrElement) -> Self {
        SsrNode::Element(value)
    }
}

#[derive(Clone, PartialEq)]
pub enum SsrEventTarget {
    Node(SsrNode),
    Window,
    Document,
}

#[derive(Clone)]
pub struct SsrEventListener {
    pub name: Str,
    pub target: SsrEventTarget,
    pub channel: Shared<Option<(async_channel::Sender<()>, async_channel::Receiver<()>)>>,
}

impl PartialEq for SsrEventListener {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.target == other.target
    }
}

impl ViewEventListener<Ssr> for SsrEventListener {
    type Event = ();

    fn next(&self) -> impl Future<Output = Self::Event> {
        self.ensure_channel();
        let channel = self.channel.get();
        let (_, rx) = channel.as_ref().unwrap();
        let rx = rx.clone();
        async move { rx.recv().await.unwrap() }
    }
}

impl SsrEventListener {
    pub fn on_window(name: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().to_owned().into(),
            target: SsrEventTarget::Window,
            channel: Default::default(),
        }
    }

    pub fn on_document(name: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().to_owned().into(),
            target: SsrEventTarget::Document,
            channel: Default::default(),
        }
    }

    pub fn new(target: SsrEventTarget, name: impl Into<Str>) -> Self {
        SsrEventListener {
            name: name.into(),
            channel: Default::default(),
            target,
        }
    }

    fn ensure_channel(&self) {
        if self.channel.get().is_none() {
            *self.channel.get_mut() = Some(async_channel::bounded(1));
        }
    }

    /// Fire an event occurence to any waiting listeners.
    pub async fn fire(&self) {
        self.ensure_channel();
        let channel = self.channel.get();
        let (tx, _) = channel.as_ref().unwrap();
        tx.send(()).await.unwrap();
    }
}

#[derive(Clone)]
pub struct Ssr;

impl View for Ssr {
    type Element = SsrElement;
    type Text = SsrText;
    type Node = SsrNode;
    type EventListener = SsrEventListener;
}

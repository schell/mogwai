//! # Server-Side Rendering (SSR)
//!
//! This module provides the implementation for server-side rendering of views.
//! It provides types and trait impls necessary to create and manipulate HTML
//! on the server side, allowing for the generation of string content that can
//! be sent to clients.
//!
//! ## Key Components
//!
//! - **[`SsrText`]**: Represents text nodes in the SSR context, allowing for text
//!   content manipulation and event handling.
//!
//! - **[`SsrElement`]**: Represents HTML elements in the SSR context, providing
//!   methods for managing attributes, styles, and child nodes.
//!
//! - **[`SsrNode`]**: An enumeration of possible node types (elements and text)
//!   used in SSR.
//!
//! - **[`SsrEventListener`]**: Handles event listening in the SSR context, enabling
//!   asynchronous event handling.
//!
//! ## Usage
//!
//! This module is intended for use in environments where server-side rendering
//! is required, providing a way to generate HTML content dynamically based on
//! application state and logic.
use std::{borrow::Cow, sync::atomic::AtomicUsize};

use crate::{
    Str,
    sync::{Global, Shared},
    view::*,
};

pub mod prelude {
    pub use super::{Ssr, SsrElement, SsrEventListener, SsrEventTarget, SsrText};
    pub use crate::prelude::*;
}

static NEXT_ID: Global<AtomicUsize> = Global::new(|| AtomicUsize::new(0));

fn next_id() -> usize {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Clone)]
pub struct SsrText {
    pub id: usize,
    pub text: Shared<Str>,
    pub events: Shared<Vec<SsrEventListener>>,
}

impl PartialEq for SsrText {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl ViewText for SsrText {
    fn new(text: impl AsRef<str>) -> Self {
        SsrText {
            id: next_id(),
            text: Shared::from_string(text),
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

#[derive(Clone)]
pub struct SsrElement {
    pub id: usize,
    pub name: Str,
    pub attributes: Shared<Vec<(Str, Option<Str>)>>,
    pub styles: Shared<Vec<(Str, Str)>>,
    pub events: Shared<Vec<SsrEventListener>>,
    pub children: Shared<Vec<SsrNode>>,
}

impl PartialEq for SsrElement {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl ViewParent<Ssr> for SsrElement {
    fn append_node(&self, node: Cow<'_, <Ssr as View>::Node>) {
        println!("appending node {}, {}", node.id(), node.name());
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

impl SsrNode {
    pub fn id(&self) -> usize {
        match self {
            SsrNode::Element(ssr_element) => ssr_element.id,
            SsrNode::Text(ssr_text) => ssr_text.id,
        }
    }

    pub fn name(&self) -> String {
        match self {
            SsrNode::Element(ssr_element) => ssr_element.name.to_string(),
            SsrNode::Text(ssr_text) => ssr_text.text.get().to_string(),
        }
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
    fn next(&self) -> impl Future<Output = ()> {
        self.ensure_channel();
        let channel = self.channel.get();
        let (_, rx) = channel.as_ref().unwrap();
        let rx = rx.clone();
        async move { rx.recv().await.unwrap() }
    }

    fn on_window(event_name: impl Into<Cow<'static, str>>) -> <Ssr as View>::EventListener {
        SsrEventListener::new(SsrEventTarget::Window, event_name)
    }

    fn on_document(event_name: impl Into<Cow<'static, str>>) -> <Ssr as View>::EventListener {
        SsrEventListener::new(SsrEventTarget::Document, event_name)
    }
}

impl SsrEventListener {
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
    type Event = ();
}

impl ViewElement for SsrElement {
    type View = Ssr;

    fn new(name: impl AsRef<str>) -> Self {
        Self {
            id: next_id(),
            name: name.as_ref().to_owned().into(),
            attributes: Default::default(),
            styles: Default::default(),
            events: Default::default(),
            children: Default::default(),
        }
    }
}

impl ViewEvent for () {
    type View = Ssr;
}

#[cfg(test)]
mod test {
    #[test]
    fn proxy_update_text_node() {
        use crate as mogwai;
        use mogwai::ssr::prelude::*;

        #[derive(Debug, PartialEq)]
        struct Status {
            color: String,
            message: String,
        }

        struct Widget<V: View> {
            root: V::Element,
            state: Proxy<Status>,
        }

        fn new_widget<V: View>() -> Widget<V> {
            let mut state = Proxy::new(Status {
                color: "black".to_string(),
                message: "Hello".to_string(),
            });

            // We start out with a `div` element bound to `root`, containing a nested `p` tag
            // with the message "Hello" in black.
            rsx! {
                let root = div() {
                    p(
                        id = "message_wrapper",
                        // proxy use in attribute position
                        style:color = state(s => &s.color)
                    ) {
                        // proxy use in node position
                        {state(s => {
                            println!("updating state to: {s:#?}");
                            &s.message
                        })}
                    }
                }
            }

            Widget { root, state }
        }

        println!("creating");
        // Verify at creation that the view shows "Hello" in black.
        let mut w = new_widget::<mogwai::ssr::Ssr>();
        assert_eq!(
            r#"<div><p id="message_wrapper" style="color: black;">Hello</p></div>"#,
            w.root.html_string()
        );

        // Then later we change the message to show "Goodbye" in red.
        w.state.set(Status {
            color: "red".to_string(),
            message: "Goodbye".to_string(),
        });
        assert_eq!(
            r#"<div><p id="message_wrapper" style="color: red;">Goodbye</p></div>"#,
            w.root.html_string()
        );
    }
}

//! Builder patterns for views.
use std::any::Any;

use crate::{Str, sync::Shared, view::*};

#[derive(Clone)]
pub struct TextBuilder {
    pub text: Shared<Str>,
    pub built: Shared<Option<Box<dyn Any + Send + Sync + 'static>>>,
}

impl PartialEq for TextBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl ViewText for TextBuilder {
    fn new(text: impl Into<Str>) -> Self {
        TextBuilder {
            text: text.into().into(),
            built: Default::default(),
        }
    }

    fn set_text(&self, text: impl Into<Str>) {
        self.text.set(text.into());
    }

    fn get_text(&self) -> Str {
        self.text.get().clone()
    }
}

impl ViewChild for TextBuilder {
    type Node = NodeBuilder;

    fn as_append_arg(&self) -> AppendArg<impl Iterator<Item = Self::Node>> {
        AppendArg::new(std::iter::once(NodeBuilder::Text(self.clone())))
    }
}

/// Builder for runtime views.
#[derive(Clone)]
pub struct ElementBuilder {
    pub name: Str,
    pub built: Shared<Option<Box<dyn Any + Send + Sync + 'static>>>,
    pub attributes: Shared<Vec<(Str, Option<Str>)>>,
    pub styles: Shared<Vec<(Str, Str)>>,
    pub events: Shared<Vec<EventListenerBuilder>>,
    pub children: Shared<Vec<NodeBuilder>>,
}

impl PartialEq for ElementBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.attributes == other.attributes
            && self.styles == other.styles
            && self.events == other.events
            && self.children == other.children
    }
}

impl ViewParent for ElementBuilder {
    type Node = NodeBuilder;

    fn remove_child(&self, child: impl ViewChild<Node = Self::Node>) {
        for child in child.as_append_arg() {
            self.children.get_mut().retain(|kid| kid != &child);
        }
    }

    fn append_child(&self, child: impl ViewChild<Node = Self::Node>) {
        let mut children = self.children.get_mut();
        children.extend(child.as_append_arg());
    }
}

impl ViewChild for ElementBuilder {
    type Node = NodeBuilder;

    fn as_append_arg(&self) -> AppendArg<impl Iterator<Item = Self::Node>> {
        AppendArg::new(std::iter::once(NodeBuilder::Element(self.clone())))
    }
}

impl ViewProperties for ElementBuilder {
    fn set_property(&self, key: impl Into<Str>, value: impl Into<Str>) {
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

    fn has_property(&self, key: impl AsRef<str>) -> bool {
        for (pkey, _pval) in self.attributes.get().iter() {
            if pkey.as_str() == key.as_ref() {
                return true;
            }
        }
        false
    }

    fn get_property(&self, key: impl AsRef<str>) -> Option<Str> {
        for (pkey, pval) in self.attributes.get().iter() {
            if pkey.as_str() == key.as_ref() {
                return pval.clone();
            }
        }
        None
    }

    fn remove_property(&self, key: impl AsRef<str>) {
        self.attributes
            .get_mut()
            .retain_mut(|p| p.0.as_str() != key.as_ref());
    }
}

impl ElementBuilder {
    pub fn new(name: impl Into<Str>) -> Self {
        Self {
            name: name.into(),
            built: Default::default(),
            attributes: Default::default(),
            styles: Default::default(),
            events: Default::default(),
            children: Default::default(),
        }
    }

    pub fn listen(&self, event_name: impl Into<Str>) -> EventListenerBuilder {
        let event_listener = EventListenerBuilder {
            name: event_name.into(),
            channel: Default::default(),
            target: EventTargetBuilder::Node(NodeBuilder::Element(self.clone())),
            built: Default::default(),
        };
        self.events.get_mut().push(event_listener.clone());
        event_listener
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

    // /// Add a child.
    // pub fn append_child(&self, child: impl Into<NodeBuilder>) {
    //     self.children.get_mut().push(child.into());
    // }

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
                let node = match kid {
                    NodeBuilder::Element(element_builder) => element_builder.html_string(),
                    NodeBuilder::Text(text_builder) => text_builder.text.get().to_string(),
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
pub enum NodeBuilder {
    Element(ElementBuilder),
    Text(TextBuilder),
}

impl From<TextBuilder> for NodeBuilder {
    fn from(value: TextBuilder) -> Self {
        NodeBuilder::Text(value)
    }
}

impl From<ElementBuilder> for NodeBuilder {
    fn from(value: ElementBuilder) -> Self {
        NodeBuilder::Element(value)
    }
}

#[derive(Clone, PartialEq)]
pub enum EventTargetBuilder {
    Node(NodeBuilder),
    Window,
    Document,
}

#[derive(Clone)]
pub struct EventListenerBuilder {
    pub name: Str,
    pub target: EventTargetBuilder,
    pub channel: Shared<Option<(async_channel::Sender<()>, async_channel::Receiver<()>)>>,
    pub built: Shared<Option<Box<dyn Any + Send + Sync + 'static>>>,
}

impl PartialEq for EventListenerBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.target == other.target
    }
}

impl ViewEventListener for EventListenerBuilder {
    type Event = ();

    fn next(&self) -> impl Future<Output = Self::Event> {
        self.ensure_channel();
        let channel = self.channel.get();
        let (_, rx) = channel.as_ref().unwrap();
        let rx = rx.clone();
        async move { rx.recv().await.unwrap() }
    }
}

impl EventListenerBuilder {
    pub fn on_window(name: impl Into<Str>) -> Self {
        Self {
            name: name.into(),
            target: EventTargetBuilder::Window,
            channel: Default::default(),
            built: Default::default(),
        }
    }

    pub fn on_document(name: impl Into<Str>) -> Self {
        Self {
            name: name.into(),
            target: EventTargetBuilder::Document,
            channel: Default::default(),
            built: Default::default(),
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
pub struct Builder;

impl View for Builder {
    type Element<T>
        = ElementBuilder
    where
        T: ViewParent + ViewChild + ViewProperties;
    type Text = TextBuilder;
    type EventListener = EventListenerBuilder;
}

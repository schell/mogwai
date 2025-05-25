//! Future of mogwai.
//!
//! ## Impetus
//!
//! What I want is the ability to define a UI element and then render it
//! with various different platforms and have it behave similarly.
//!
//! An example of this would be creating a button that shows the number of
//! times it has been clicked, and then deploying that on the web, as a server-side
//! rendered string (after appying some number of artificial clicks) and also deploying
//! it in a terminal as a TUI.
//!
//! We might accomplish this with bare-bones Rust by defining the element in terms of a
//! model and a view interface. The model encodes the local state of the element
//! and its runtime logic, while the view interface determines how the runtime
//! logic can affect the view.
//!
//! The model is some concrete type, like `struct ButtonClicks {..}` and the view interface
//! would be a trait, `pub trait ButtonClicksInterface {..}`.
//!
//! Then each view platform ("web", "tui" and "ssr" in our case) could implement the view
//! interface and define the entry point.
//!
//! Model+logic and view.
//!
//! ### Model
//! Model is some concrete data that is used to update the view.
//! The type of the model cannot change from platform to platform.
//!
//! ### View Interface
//! A trait for interacting with the view in a cross-platform way.
//!
//! ### Logic
//! The logic is the computation that takes changes from the view through the interface,
//! updates the model and applies changes back through the interface.
//!
//! ### View
//! The view itself is responsible for rendering and providing events to the logic.
//! The type of the view changes depending on the platform.
//!
//! ## Strategy
//!
//! Mogwai's strategy towards solving the problem of cross-platform UI is not to offer
//! a one-size fits all view solution. Instead, `mogwai` aims to aid a _disciplined_
//! developer in modelling the UI using traits, and then providing the developer with
//! tools and wrappers to make fullfilling those traits on specific platforms as easy
//! as possible.

use std::{any::Any, borrow::Cow};

use sync::Shared;

pub mod macros;
pub mod sync;
pub mod tuple;
#[cfg(feature = "web")]
pub mod web;

pub mod prelude {
    pub use crate::{
        Builder, ElementBuilder, EventListenerBuilder, NodeBuilder, TextBuilder, View,
        ViewContainer, ViewEventListener, ViewNode, ViewText,
    };
}

/// A transparent wrapper around [`Cow<'static, str>`].
#[repr(transparent)]
#[derive(Clone, Default)]
pub struct Str {
    inner: Cow<'static, str>,
}

impl core::fmt::Display for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}

impl From<&'static str> for Str {
    fn from(s: &'static str) -> Self {
        Str { inner: s.into() }
    }
}

impl From<String> for Str {
    fn from(s: String) -> Self {
        Str { inner: s.into() }
    }
}

impl<'a> From<&'a String> for Str {
    fn from(s: &'a String) -> Self {
        Str {
            inner: s.clone().into(),
        }
    }
}

impl From<Cow<'static, str>> for Str {
    fn from(inner: Cow<'static, str>) -> Self {
        Str { inner }
    }
}

impl<'a> From<&'a Cow<'static, str>> for Str {
    fn from(s: &'a Cow<'static, str>) -> Self {
        Str { inner: s.clone() }
    }
}

impl Str {
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

pub trait ViewText {
    fn new(text: impl Into<Str>) -> Self;
    fn set_text(&self, text: impl Into<Str>);
}

impl ViewText for Shared<Str> {
    fn new(text: impl Into<Str>) -> Self {
        Shared::from(text.into())
    }

    fn set_text(&self, text: impl Into<Str>) {
        self.set(text.into());
    }
}

impl ViewText for TextBuilder {
    fn new(text: impl Into<Str>) -> Self {
        TextBuilder::new(text)
    }

    fn set_text(&self, text: impl Into<Str>) {
        self.text.set(text.into());
    }
}

#[cfg(feature = "web")]
impl ViewText for web_sys::Text {
    fn new(text: impl Into<Str>) -> Self {
        web_sys::Text::new_with_data(&text.into().inner).unwrap()
    }

    fn set_text(&self, text: impl Into<Str>) {
        let text = text.into();
        self.set_data(&text.inner);
    }
}

pub trait ViewEventListener {
    type Event;

    fn next(&self) -> impl Future<Output = Self::Event>;
}

pub trait ViewNode {
    type Parent<T>: ViewContainer;

    fn append_to_parent<T>(&self, parent: impl AsRef<Self::Parent<T>>);
}

pub trait ViewContainer {
    fn append_child<C, T>(&self, child: &C)
    where
        C: ViewNode,
        Self: AsRef<C::Parent<T>>,
    {
        child.append_to_parent(self);
    }
}

#[derive(Clone)]
pub struct TextBuilder {
    text: Shared<Str>,
    built: Shared<Option<Box<dyn Any>>>,
}

impl ViewNode for TextBuilder {
    type Parent<T> = ElementBuilder;

    fn append_to_parent<T>(&self, parent: impl AsRef<Self::Parent<T>>) {
        parent
            .as_ref()
            .children
            .get_mut()
            .push(NodeBuilder::Text(self.clone()));
    }
}

impl TextBuilder {
    pub fn new(text: impl Into<Str>) -> Self {
        Self {
            text: text.into().into(),
            built: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct EventListenerBuilder {
    name: Str,
    node: NodeBuilder,
    built: Shared<Option<Box<dyn Any>>>,
}

impl ViewEventListener for EventListenerBuilder {
    type Event = ();

    fn next(&self) -> impl Future<Output = Self::Event> {
        std::future::ready(())
    }
}

/// Builder for runtime views.
#[derive(Clone)]
pub struct ElementBuilder {
    name: Str,
    built: Shared<Option<Box<dyn Any>>>,
    attributes: Shared<Vec<(Str, Option<Str>)>>,
    styles: Shared<Vec<(Str, Str)>>,
    events: Shared<Vec<EventListenerBuilder>>,
    children: Shared<Vec<NodeBuilder>>,
}

impl AsRef<ElementBuilder> for ElementBuilder {
    fn as_ref(&self) -> &ElementBuilder {
        self
    }
}

impl ViewNode for ElementBuilder {
    type Parent<T> = ElementBuilder;

    fn append_to_parent<T>(&self, parent: impl AsRef<Self::Parent<T>>) {
        parent
            .as_ref()
            .children
            .get_mut()
            .push(NodeBuilder::Element(self.clone()));
    }
}

impl ViewContainer for ElementBuilder {}

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
            node: NodeBuilder::Element(self.clone()),
            built: Default::default(),
        };
        self.events.get_mut().push(event_listener.clone());
        event_listener
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

#[derive(Clone)]
pub enum NodeBuilder {
    Element(ElementBuilder),
    Text(TextBuilder),
}

impl ViewNode for NodeBuilder {
    type Parent = ElementBuilder;

    fn append_to_parent(&self, parent: impl AsRef<Self::Parent>) {
        parent.as_ref().children.get_mut().push(self.clone());
    }
}

// impl From<&TextBuilder> for NodeBuilder {
//     fn from(value: &TextBuilder) -> Self {
//         NodeBuilder::Text(value.clone())
//     }
// }

// impl From<&ElementBuilder> for NodeBuilder {
//     fn from(value: &ElementBuilder) -> Self {
//         NodeBuilder::Element(value.clone())
//     }
// }

pub trait View {
    type Node: ViewNode;
    type Element<T>: ViewContainer
    where
        T: ViewContainer;
    // TODO: revisit to see if we need this `T`
    type Text<T>: ViewText
    where
        T: ViewText;
    // TODO: revisit to see if we need this `T`
    type EventListener<T>: ViewEventListener
    where
        T: ViewEventListener;
}

pub struct Builder;

impl View for Builder {
    type Node = NodeBuilder;
    type Element<T>
        = ElementBuilder
    where
        T: ViewContainer;
    type Text<T>
        = TextBuilder
    where
        T: ViewText;
    type EventListener<T>
        = EventListenerBuilder
    where
        T: ViewEventListener;
}

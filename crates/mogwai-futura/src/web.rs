//! Utilities for web (through web-sys).

use std::{
    fmt::Write,
    ops::{Deref, DerefMut},
    sync::{Arc, atomic::AtomicUsize},
};

use event::EventListener;
use send_wrapper::SendWrapper;
use web_sys::wasm_bindgen::JsCast;

use crate::prelude::*;
pub mod event;

pub use mogwai_future_rsx::rsx;

pub mod prelude {
    pub use super::rsx;
    pub use super::{Web, event::*};
    pub use crate::prelude::*;
}

impl ViewChild for web_sys::Node {
    type Node = web_sys::Node;

    fn as_child(&self) -> Self::Node {
        self.clone()
    }
}

impl ViewParent for web_sys::Node {
    type Node = web_sys::Node;

    fn append_child(&self, child: &impl ViewChild<Node = Self::Node>) {
        let child = child.as_child();
        self.append_child(&child).unwrap();
    }
}

macro_rules! node_impl {
    ($ty:ty) => {
        impl ViewChild for $ty {
            type Node = web_sys::Node;

            fn as_child(&self) -> Self::Node {
                let child: &web_sys::Node = self.as_ref();
                child.clone()
            }
        }

        impl ViewParent for $ty {
            type Node = web_sys::Node;

            fn append_child(&self, child: &impl ViewChild<Node = Self::Node>) {
                let child = child.as_child();
                web_sys::Node::append_child(&self.as_ref(), &child).unwrap();
            }
        }
    };
}

node_impl!(web_sys::Text);
node_impl!(web_sys::Element);
node_impl!(web_sys::HtmlElement);

impl ViewText for web_sys::Text {
    fn new(text: impl Into<Str>) -> Self {
        web_sys::Text::new_with_data(text.into().as_str()).unwrap()
    }

    fn set_text(&self, text: impl Into<Str>) {
        let text = text.into();
        self.set_data(text.as_str());
    }
}

impl ViewEventListener for EventListener {
    type Event = web_sys::Event;

    fn next(&self) -> impl Future<Output = Self::Event> {
        self.next()
    }
}

pub struct Web;

impl View for Web {
    type Element<T>
        = T
    where
        T: ViewParent + ViewChild;
    type Text = web_sys::Text;
    type EventListener = EventListener;
}

static PAD: std::sync::LazyLock<Arc<AtomicUsize>> = std::sync::LazyLock::new(|| Arc::new(0.into()));

struct Pad(usize);

impl Drop for Pad {
    fn drop(&mut self) {
        PAD.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl core::fmt::Display for Pad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for _ in 0..2 * self.0 {
            f.write_char(' ')?;
        }
        Ok(())
    }
}

impl Pad {
    fn new() -> Self {
        let n = PAD.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        Pad(n)
    }
}

impl Web {
    pub fn build_text(builder: TextBuilder) -> web_sys::Text {
        let pad = Pad::new();
        log::trace!("{pad}building text: {}", builder.text.get().deref());
        if let Some(already_built) = builder.built.get().as_ref() {
            log::trace!("{pad}already built");
            // UNWRAP: safe because only this function ever sets `built`
            already_built
                .downcast_ref::<SendWrapper<web_sys::Text>>()
                .unwrap()
                .deref()
                .clone()
        } else {
            log::trace!("{pad}fresh build of text");
            let built = web_sys::Text::new_with_data(builder.text.get().as_str()).unwrap();
            builder
                .built
                .set(Some(Box::new(SendWrapper::new(built.clone()))));
            built
        }
    }

    pub fn build_listener(builder: EventListenerBuilder) -> EventListener {
        let pad = Pad::new();
        log::trace!("{pad}building listener: {}", builder.name);
        if let Some(already_built) = builder.built.get().as_ref() {
            log::trace!("{pad}already built listener");
            already_built
                .downcast_ref::<SendWrapper<EventListener>>()
                .unwrap()
                .deref()
                .clone()
        } else {
            log::trace!("{pad}fresh build of listener");
            let listener = match builder.target {
                EventTargetBuilder::Window => {
                    log::trace!("{pad}must first get the window");
                    EventListener::new(web_sys::window().unwrap(), builder.name)
                }
                EventTargetBuilder::Document => {
                    log::trace!("{pad}must first get the document");
                    EventListener::new(web_sys::window().unwrap().document().unwrap(), builder.name)
                }
                EventTargetBuilder::Node(node) => match node {
                    NodeBuilder::Element(element_builder) => {
                        log::trace!("{pad}must first build the element target");
                        let element = Self::build_element::<web_sys::Element>(element_builder);
                        EventListener::new(&element, builder.name)
                    }
                    NodeBuilder::Text(text_builder) => {
                        log::trace!("{pad}must first build the text target");
                        let text = Self::build_text(text_builder);
                        EventListener::new(&text, builder.name)
                    }
                },
            };
            builder
                .built
                .set(Some(Box::new(SendWrapper::new(listener.clone()))));
            log::trace!("{pad}built listener");
            listener
        }
    }

    pub fn build_element<T: Clone + JsCast + 'static>(builder: ElementBuilder) -> T {
        let pad = Pad::new();
        log::trace!("{pad}building element: {}", builder.name);
        let ElementBuilder {
            name,
            built,
            attributes,
            styles,
            events,
            children,
        } = builder;
        if let Some(already_built) = built.get().as_ref() {
            log::trace!("{pad}already built element");
            let element = already_built
                .downcast_ref::<SendWrapper<web_sys::Element>>()
                .unwrap()
                .deref()
                .clone();
            return element.dyn_into::<T>().unwrap();
        }

        log::trace!("{pad}fresh build of element");
        let mut maybe_ns = None;
        attributes.get_mut().retain_mut(|(k, may_v)| {
            if k.as_str() == "xmlns" {
                maybe_ns = may_v.take();
                false
            } else {
                true
            }
        });
        let el = if let Some(ns) = maybe_ns {
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .create_element_ns(Some(ns.as_str()), name.as_str())
                .unwrap()
        } else {
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .create_element(name.as_str())
                .unwrap()
        };
        // Set the built element first, so we don't recurse when building event targets.
        built.set(Some(Box::new(SendWrapper::new(el.clone()))));

        for (k, may_v) in std::mem::take(attributes.get_mut().deref_mut()).into_iter() {
            log::trace!("{pad}set att {k} = {}", may_v.as_deref().unwrap_or("none"));
            let value = may_v.unwrap_or_else(|| "".into());
            el.dyn_ref::<web_sys::Element>()
                .unwrap()
                .set_attribute(k.as_str(), value.as_str())
                .unwrap();
        }
        for (k, v) in std::mem::take(styles.get_mut().deref_mut()).into_iter() {
            log::trace!("{pad}set style {k} = {v}");
            let style = el.dyn_ref::<web_sys::HtmlElement>().unwrap().style();
            style.set_property(k.as_str(), v.as_str()).unwrap();
        }
        for event_builder in std::mem::take(events.get_mut().deref_mut()).into_iter() {
            log::trace!("{pad}listener");
            // We don't have to do anything with the listener except build it.
            let _listener = Self::build_listener(event_builder);
        }
        for (i, child) in std::mem::take(children.get_mut().deref_mut())
            .into_iter()
            .enumerate()
        {
            log::trace!("{pad}child {i}");
            let node = match child {
                NodeBuilder::Text(text_builder) => {
                    let text = Self::build_text(text_builder);
                    text.dyn_into::<web_sys::Node>().unwrap()
                }
                NodeBuilder::Element(element_builder) => {
                    Self::build_element::<web_sys::Node>(element_builder)
                }
            };
            el.dyn_ref::<web_sys::Node>()
                .unwrap()
                .append_child(&node)
                .unwrap();
        }
        log::trace!("{pad}built: {}", name);
        el.dyn_into::<T>().unwrap()
    }
}

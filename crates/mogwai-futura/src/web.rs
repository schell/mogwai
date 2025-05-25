//! Utilities for web (through web-sys).

use std::ops::DerefMut;

use event::EventListener;
use web_sys::wasm_bindgen::JsCast;

use crate::{ElementBuilder, EventListenerBuilder, NodeBuilder, TextBuilder};
pub mod event;

pub mod prelude {
    pub use crate::{
        Builder, Container, ElementBuilder, EventListenerBuilder, NodeBuilder, TextBuilder, View,
        ViewText,
    };

    pub use super::{Web, event::*};
}

pub struct Web;

impl super::View for Web {
    type Element<T> = T;
    type Text<T> = T;
    type EventListener<T> = EventListener;
}

impl Web {
    pub fn build_text(builder: TextBuilder) -> web_sys::Text {
        if let Some(already_built) = builder.built.get().as_ref() {
            // UNWRAP: safe because only this function ever sets `built`
            already_built
                .downcast_ref::<web_sys::Text>()
                .unwrap()
                .clone()
        } else {
            let built = web_sys::Text::new_with_data(builder.text.get().as_str()).unwrap();
            builder.built.set(Some(Box::new(built.clone())));
            built
        }
    }

    pub fn build_listener(builder: EventListenerBuilder) -> EventListener {
        if let Some(already_built) = builder.built.get().as_ref() {
            already_built
                .downcast_ref::<EventListener>()
                .unwrap()
                .clone()
        } else {
            let listener = match builder.node {
                NodeBuilder::Element(element_builder) => {
                    let element = Self::build_element::<web_sys::Element>(element_builder);
                    EventListener::new(&element, builder.name)
                }
                NodeBuilder::Text(text_builder) => {
                    let text = Self::build_text(text_builder);
                    EventListener::new(&text, builder.name)
                }
            };
            builder.built.set(Some(Box::new(listener.clone())));
            listener
        }
    }

    pub fn build_element<T: Clone + JsCast + 'static>(builder: ElementBuilder) -> T {
        let ElementBuilder {
            name,
            built,
            attributes,
            styles,
            events,
            children,
        } = builder;
        if let Some(already_built) = built.get().as_ref() {
            return already_built.downcast_ref::<T>().unwrap().clone();
        }
        let el = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element(name.as_str())
            .unwrap()
            .dyn_into::<T>()
            .unwrap();
        for (k, may_v) in std::mem::take(attributes.get_mut().deref_mut()).into_iter() {
            let value = may_v.unwrap_or_else(|| "".into());
            el.dyn_ref::<web_sys::Element>()
                .unwrap()
                .set_attribute(k.as_str(), value.as_str())
                .unwrap();
        }
        for (k, v) in std::mem::take(styles.get_mut().deref_mut()).into_iter() {
            let style = el.dyn_ref::<web_sys::HtmlElement>().unwrap().style();
            style.set_property(k.as_str(), v.as_str()).unwrap();
        }
        for event_builder in std::mem::take(events.get_mut().deref_mut()).into_iter() {
            // We don't have to do anything with the listener except build it.
            let _listener = Self::build_listener(event_builder);
        }
        for child in std::mem::take(children.get_mut().deref_mut()).into_iter() {
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
        built.set(Some(Box::new(el.clone())));
        el
    }
}

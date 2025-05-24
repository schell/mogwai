//! Utilities for web (through web-sys).

use std::ops::DerefMut;

use event::{EventListener, EventListenerExt};
use web_sys::wasm_bindgen::JsCast;

use crate::{ElementBuilder, EventListenerBuilder, NodeBuilder, TextBuilder};
pub mod event;

pub mod prelude {
    pub use crate::{
        Builder, Container, ElementBuilder, EventListenerBuilder, NodeBuilder, TextBuilder, View,
        ViewText, Web,
    };

    pub use super::event::*;
}

impl<T: Clone + JsCast> TextBuilder<T> {
    pub fn build_web(self) -> T {
        if let Some(already_built) = self.built.get().as_ref() {
            already_built.clone()
        } else {
            let built = web_sys::Text::new_with_data(self.text.get().as_str())
                .unwrap()
                .dyn_into::<T>()
                .unwrap();
            self.built.set(Some(built.clone()));
            built
        }
    }
}

impl EventListenerBuilder<EventListener> {
    pub fn build_web(self, event_target: &web_sys::EventTarget) -> EventListener {
        if let Some(already_built) = self.built.get().as_ref() {
            already_built.clone()
        } else {
            let listener = event_target.listen(self.name);
            self.built.set(Some(listener.clone()));
            listener
        }
    }
}

impl<T: Clone + JsCast> ElementBuilder<T, web_sys::Text, EventListener> {
    fn build_web(self) -> T {
        let ElementBuilder {
            name,
            built,
            attributes,
            styles,
            events,
            children,
        } = self;
        if let Some(already_built) = built.get().as_ref() {
            return already_built.clone();
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
            let _listener = event_builder.build_web(el.dyn_ref::<web_sys::EventTarget>().unwrap());
        }
        for child in std::mem::take(children.get_mut().deref_mut()).into_iter() {
            let node = match child {
                NodeBuilder::Text(text_builder) => {
                    let text = text_builder.build_web();
                    text.dyn_into::<web_sys::Node>().unwrap()
                }
                NodeBuilder::Element(element_builder) => {
                    let child_el = element_builder.build_web();
                    child_el
                        .dyn_into::<web_sys::Node>()
                        .unwrap_or_else(|_| panic!("cannot cast to node"))
                }
            };
            el.dyn_ref::<web_sys::Node>()
                .unwrap()
                .append_child(&node)
                .unwrap();
        }
        el
    }
}

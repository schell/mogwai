//! Utilities for web (through web-sys).

use std::{
    cell::RefCell,
    fmt::Write,
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::{Arc, atomic::AtomicUsize},
    task::Waker,
};

use event::EventListener;
use send_wrapper::SendWrapper;
use wasm_bindgen::{JsValue, UnwrapThrowExt, prelude::Closure};
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

    fn as_append_arg(&self) -> AppendArg<impl Iterator<Item = Self::Node>> {
        AppendArg::new(std::iter::once(self.clone()))
    }
}

impl ViewParent for web_sys::Node {
    type Node = web_sys::Node;

    fn remove_child(&self, child: impl ViewChild<Node = Self::Node>) {
        for child in child.as_append_arg() {
            let _ = self.remove_child(&child);
        }
    }

    fn append_child(&self, child: impl ViewChild<Node = Self::Node>) {
        for child in child.as_append_arg() {
            let _ = self.append_child(&child);
        }
    }
}

macro_rules! node_impl {
    ($ty:ident, $from:ty, $fn:ident) => {
        impl ViewChild for web_sys::$ty {
            type Node = web_sys::Node;

            fn as_append_arg(&self) -> AppendArg<impl Iterator<Item = Self::Node>> {
                let node: &web_sys::Node = self.as_ref();
                AppendArg::new(std::iter::once(node.clone()))
            }
        }

        impl ViewParent for web_sys::$ty {
            type Node = web_sys::Node;

            fn remove_child(&self, child: impl ViewChild<Node = Self::Node>) {
                for child in child.as_append_arg() {
                    let _ = web_sys::Node::remove_child(self, &child);
                }
            }

            fn append_child(&self, child: impl ViewChild<Node = Self::Node>) {
                for child in child.as_append_arg() {
                    let _ = web_sys::Node::append_child(self, &child);
                }
            }
        }

        impl From<$from> for web_sys::$ty {
            fn from(builder: $from) -> Self {
                Web::$fn(builder)
            }
        }
    };

    ($ty:ident, $from:ty, $fn:ident, props) => {
        node_impl!($ty, $from, $fn);

        impl ViewProperties for web_sys::$ty {
            fn set_property(&self, key: impl Into<Str>, value: impl Into<Str>) {
                let _ = self.set_attribute(key.into().as_str(), value.into().as_str());
            }

            fn has_property(&self, key: impl AsRef<str>) -> bool {
                self.has_attribute(key.as_ref())
            }

            fn get_property(&self, key: impl AsRef<str>) -> Option<Str> {
                self.get_attribute(key.as_ref()).map(|s| s.into())
            }

            fn remove_property(&self, key: impl AsRef<str>) {
                let _ = self.remove_attribute(key.as_ref());
            }
        }
    };
}

node_impl!(Text, TextBuilder, build_text);
node_impl!(Element, ElementBuilder, build_element, props);
node_impl!(HtmlElement, ElementBuilder, build_element, props);
node_impl!(HtmlInputElement, ElementBuilder, build_element, props);

impl ViewText for web_sys::Text {
    fn new(text: impl Into<Str>) -> Self {
        web_sys::Text::new_with_data(text.into().as_str()).unwrap()
    }

    fn set_text(&self, text: impl Into<Str>) {
        let text = text.into();
        self.set_data(text.as_str());
    }

    fn get_text(&self) -> Str {
        self.data().into()
    }
}

impl ViewEventListener for EventListener {
    type Event = web_sys::Event;

    fn next(&self) -> impl Future<Output = Self::Event> {
        self.next()
    }
}

#[derive(Clone, Copy)]
pub struct Web;

impl View for Web {
    type Element<T>
        = T
    where
        T: ViewParent + ViewChild + ViewProperties;
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

thread_local! {
    pub static WINDOW: web_sys::Window = web_sys::window().unwrap_throw();
    pub static DOCUMENT: web_sys::Document = WINDOW.with(|w| w.document().unwrap_throw());
}

/// Return the DOM [`web_sys::Window`].
/// #### Panics
/// Panics when the window cannot be returned.
pub fn window() -> web_sys::Window {
    WINDOW.with(|w| w.clone())
}

/// Return the document JsDom object [`web_sys::Document`]
/// #### Panics
/// Panics on non-wasm32 or when the document cannot be returned.
pub fn document() -> web_sys::Document {
    DOCUMENT.with(|d| d.clone())
}

/// Return the body Dom object.
///
/// ## Panics
/// Panics on wasm32 if the body cannot be returned.
pub fn body() -> web_sys::HtmlElement {
    DOCUMENT.with(|d| d.body().expect("document does not have a body"))
}

fn req_animation_frame(f: &Closure<dyn FnMut(JsValue)>) {
    WINDOW.with(|w| {
        w.request_animation_frame(f.as_ref().unchecked_ref())
            .expect("should register `requestAnimationFrame` OK")
    });
}

#[derive(Clone, Default)]
#[expect(clippy::type_complexity, reason = "not too complex")]
pub struct NextFrame {
    closure: Rc<RefCell<Option<Closure<dyn FnMut(JsValue)>>>>,
    ts: Rc<RefCell<Option<f64>>>,
    waker: Rc<RefCell<Option<Waker>>>,
}

/// Sets a static rust closure to be called with `window.requestAnimationFrame`.
/// The given function may return whether or not this function should be
/// rescheduled. If the function returns `true` it will be rescheduled.
/// Otherwise it will not. The static rust closure takes one parameter which is
/// a timestamp representing the number of milliseconds since the application's
/// load. See <https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp>
/// for more info.
pub fn request_animation_frame() -> NextFrame {
    // https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html#srclibrs
    let frame = NextFrame::default();

    *frame.closure.borrow_mut() = Some(Closure::wrap(Box::new({
        let frame = frame.clone();
        move |ts_val: JsValue| {
            *frame.ts.borrow_mut() = Some(ts_val.as_f64().unwrap_or(0.0));
            if let Some(waker) = frame.waker.borrow_mut().take() {
                waker.wake();
            }
        }
    }) as Box<dyn FnMut(JsValue)>));

    req_animation_frame(frame.closure.borrow().as_ref().unwrap_throw());

    frame
}

impl Future for NextFrame {
    type Output = f64;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Some(ts) = self.ts.borrow_mut().take() {
            std::task::Poll::Ready(ts)
        } else {
            *self.waker.borrow_mut() = Some(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}

//! # `web-sys` view implementation
//!
//! This module provides an implementation of [`View`] for [`web-sys`] types,
//! allowing for the creation and manipulation of DOM nodes in a browser environment.
//!
//! ## Key Components
//!
//! - **ViewChild**: Implements the [`ViewChild`] trait for `web-sys` types, enabling them to be
//!   appended to views.
//!
//! - **ViewParent**: Implements the [`ViewParent`] trait for `web-sys` types, providing methods
//!   for managing child nodes within a view.
//!
//! - **ViewProperties**: Implements the [`ViewProperties`] trait for various web elements, allowing
//!   for the manipulation of attributes and styles.
//!
//! - **ViewEventListener**: Implements the [`ViewEventListener`] trait for [`EventListener`], enabling
//!   asynchronous event handling.
//!
//! - **Extension traits**: [`WebElement`] and [`WebEvent`] make it easy to specialize on web views.
use std::{cell::RefCell, ops::Deref, rc::Rc, task::Waker};

use event::EventListener;
use wasm_bindgen::{JsValue, UnwrapThrowExt, prelude::Closure};
use web_sys::wasm_bindgen::JsCast;

use crate::{prelude::*, sync::Global};
pub mod event;

pub mod prelude {
    //! Re-export of the common prelude with browser specific extras.
    pub use super::{Web, WebElement, WebEvent, event::*};
    pub use crate::prelude::*;
    pub extern crate wasm_bindgen;
    pub extern crate wasm_bindgen_futures;
    pub extern crate web_sys;
}

macro_rules! node_impl {
    ($ty:ident) => {
        impl ViewEventTarget<Web> for web_sys::$ty {
            fn listen(&self, event_name: impl Into<Str>) -> EventListener {
                EventListener::new(self, event_name)
            }
        }

        impl ViewChild<Web> for web_sys::$ty {
            fn as_append_arg(
                &self,
            ) -> AppendArg<Web, impl Iterator<Item = std::borrow::Cow<'_, web_sys::Node>>> {
                AppendArg::new(std::iter::once(std::borrow::Cow::Borrowed(self.as_ref())))
            }
        }

        impl ViewParent<Web> for web_sys::$ty {
            fn append_node(&self, node: std::borrow::Cow<'_, <Web as View>::Node>) {
                web_sys::Node::append_child(self, node.as_ref()).unwrap_throw();
            }

            fn remove_node(&self, node: std::borrow::Cow<'_, <Web as View>::Node>) {
                web_sys::Node::remove_child(self, node.as_ref()).unwrap_throw();
            }

            fn replace_node(
                &self,
                new_node: std::borrow::Cow<'_, <Web as View>::Node>,
                old_node: std::borrow::Cow<'_, <Web as View>::Node>,
            ) {
                web_sys::Node::replace_child(self, new_node.as_ref(), old_node.as_ref())
                    .unwrap_throw();
            }

            fn insert_node_before(
                &self,
                new_node: std::borrow::Cow<'_, <Web as View>::Node>,
                before_node: Option<std::borrow::Cow<'_, <Web as View>::Node>>,
            ) {
                web_sys::Node::insert_before(self, new_node.as_ref(), before_node.as_deref())
                    .unwrap_throw();
            }
        }
    };
}

node_impl!(Text);
node_impl!(Node);
node_impl!(Element);

impl ViewProperties for web_sys::Element {
    fn set_property(&self, key: impl AsRef<str>, value: impl AsRef<str>) {
        let _ = self.set_attribute(key.as_ref(), value.as_ref());
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

    fn set_style(&self, key: impl AsRef<str>, value: impl AsRef<str>) {
        if let Some(el) = self.dyn_ref::<web_sys::HtmlElement>() {
            let style = el.style();
            let _ = style.set_property(key.as_ref(), value.as_ref());
        }
    }

    fn remove_style(&self, key: impl AsRef<str>) {
        if let Some(el) = self.dyn_ref::<web_sys::HtmlElement>() {
            let style = el.style();
            let _ = style.remove_property(key.as_ref());
        }
    }
}

impl ViewText for web_sys::Text {
    fn new(text: impl AsRef<str>) -> Self {
        web_sys::Text::new_with_data(text.as_ref()).unwrap()
    }

    fn set_text(&self, text: impl AsRef<str>) {
        self.set_data(text.as_ref());
    }

    fn get_text(&self) -> Str {
        self.data().into()
    }
}

impl ViewEventListener<Web> for EventListener {
    fn next(&self) -> impl Future<Output = web_sys::Event> {
        self.next()
    }

    fn on_window(event_name: impl Into<Str>) -> EventListener {
        EventListener::new(window(), event_name)
    }

    fn on_document(event_name: impl Into<Str>) -> EventListener {
        EventListener::new(document(), event_name)
    }
}

impl ViewElement for web_sys::Element {
    type View = Web;

    fn new(name: impl AsRef<str>) -> Self {
        DOCUMENT
            .create_element(name.as_ref())
            .unwrap_throw()
            .dyn_into()
            .unwrap()
    }

    fn new_namespace(name: impl AsRef<str>, ns: impl AsRef<str>) -> Self {
        DOCUMENT
            .create_element_ns(Some(ns.as_ref()), name.as_ref())
            .unwrap_throw()
            .dyn_into()
            .unwrap()
    }
}

impl ViewEvent for web_sys::Event {
    type View = Web;
}

#[derive(Clone, Copy)]
pub struct Web;

impl View for Web {
    type Element = web_sys::Element;
    type Text = web_sys::Text;
    type Node = web_sys::Node;
    type EventListener = EventListener;
    type Event = web_sys::Event;
}

static WINDOW: Global<web_sys::Window> = Global::new(|| web_sys::window().unwrap_throw());
static DOCUMENT: Global<web_sys::Document> = Global::new(|| WINDOW.document().unwrap_throw());

/// Return the DOM [`web_sys::Window`].
/// #### Panics
/// Panics when the window cannot be returned.
pub fn window() -> &'static web_sys::Window {
    WINDOW.deref()
}

/// Returns the global document object [`web_sys::Document`]
///
/// #### Panics
/// Panics on non-wasm32 or when the document cannot be returned.
pub fn document() -> &'static web_sys::Document {
    DOCUMENT.deref()
}

/// Return the global document's body object.
///
/// ## Panics
/// Panics on wasm32 if the body cannot be returned.
pub fn body() -> web_sys::HtmlElement {
    DOCUMENT
        .body()
        .expect_throw("document does not have a body")
}

fn req_animation_frame(f: &Closure<dyn FnMut(JsValue)>) {
    WINDOW
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect_throw("should register `requestAnimationFrame` OK");
}

/// Sets a static rust closure to be called with `window.requestAnimationFrame`.
///
/// The static rust closure takes one parameter which is
/// a timestamp representing the number of milliseconds since the application's
/// load. See <https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp>
/// for more info.
pub fn request_animation_frame(mut f: impl FnMut(JsValue) + 'static) {
    let wrapper = Rc::new(RefCell::new(None));
    let callback = Box::new({
        let wrapper = wrapper.clone();
        move |jsval| {
            f(jsval);
            wrapper.borrow_mut().take();
        }
    }) as Box<dyn FnMut(JsValue)>;
    let closure: Closure<dyn FnMut(JsValue)> = Closure::wrap(callback);
    *wrapper.borrow_mut() = Some(closure);
    req_animation_frame(wrapper.borrow().as_ref().unwrap_throw());
}

#[derive(Clone, Default)]
#[expect(clippy::type_complexity, reason = "not too complex")]
struct NextFrame {
    closure: Rc<RefCell<Option<Closure<dyn FnMut(JsValue)>>>>,
    ts: Rc<RefCell<Option<f64>>>,
    waker: Rc<RefCell<Option<Waker>>>,
}

/// Creates a future that will resolve on the next animation frame.
///
/// The future's output is a timestamp representing the number of
/// milliseconds since the application's load.
/// See <https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp>
/// for more info.
pub fn next_animation_frame() -> impl Future<Output = f64> {
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

/// Marker trait for specializing generic view elements to [`web_sys::Element`] and friends.
pub trait WebElement: ViewElement {
    /// Attempt to cast the element.
    ///
    /// If successful, run the given function on the result of the cast.
    fn dyn_el<T: JsCast, X>(&self, f: impl FnOnce(&T) -> X) -> Option<X> {
        let opt_x = self.when_element::<Web, _>(|el: &web_sys::Element| -> Option<X> {
            let el = el.dyn_ref::<T>()?;
            let x = f(el);
            Some(x)
        });
        opt_x.flatten()
    }
}

impl<T: ViewElement> WebElement for T {}

/// Marker trait for specializing generic view events to [`web_sys::Event`] and friends.
pub trait WebEvent: ViewEvent {
    /// Attempt to cast the element.
    ///
    /// If successful, run the given function on the result of the cast.
    fn dyn_ev<T: JsCast, X>(&self, f: impl FnOnce(&T) -> X) -> Option<X> {
        let opt_x = self.when_event::<Web, _>(|el: &web_sys::Event| -> Option<X> {
            let el = el.dyn_ref::<T>()?;
            let x = f(el);
            Some(x)
        });
        opt_x.flatten()
    }
}

impl<T: ViewEvent> WebEvent for T {}

#[cfg(test)]
mod test {
    use crate::{self as mogwai, proxy::Proxy, ssr::Ssr};
    use mogwai::web::prelude::*;

    #[test]
    /// ```compile_fail
    /// fn rsx_proxy_on_outermost_block_is_compiler_error() {
    ///     fn view<V: View>() {
    ///         let proxy = Proxy::<Web, ()>::default();
    ///         rsx! {
    ///             {proxy(() => "Erroring text node.".into_text::<Web>())}
    ///         }
    ///     }
    /// }
    /// ```
    fn rsx_doc_proxy_on_outermost_block_is_compiler_error() {}

    #[test]
    #[allow(dead_code)]
    fn rsx_unique_names() {
        struct MyView<V: View> {
            wrapper: V::Element,
        }

        fn view<V: View>() -> MyView<V> {
            rsx! {
                let wrapper = main() {
                    div() {
                        "Text one."
                        "Text two."
                        "Text three."
                        p() {
                            "Inside p one."
                        }
                        p() {
                            "Inside p two."
                        }
                    }
                }
            }
            MyView { wrapper }
        }
    }

    #[test]
    #[allow(dead_code)]
    fn rsx_block_nesting() {
        struct MyView<V: View> {
            wrapper: V::Element,
            child: MyChild<V>,
            text: V::Text,
            proxy: Proxy<Str>,
        }

        #[derive(ViewChild)]
        struct MyChild<V: View> {
            #[child]
            wrapper: V::Element,
        }

        fn view<V: View>() -> MyView<V> {
            rsx! {
                let wrapper = p() {
                    "Here lies davey jones."
                }
            }

            let child = MyChild { wrapper };

            let mut proxy = Proxy::<Str>::default();

            rsx! {
                let wrapper = div(id = "wrapper") {
                    // You can nest view structs
                    let child = {child}

                    "Constant text can live inline."

                    // You can use Rust expressions inside a block...
                    let text = {
                        "But Rust expressions in a block must evaluate to some kind of node.".into_text::<V>()
                    }

                    ul() {
                        li() {
                            "And you can use `Proxy` to insert a variable number of nodes..."
                        }
                        { proxy(s => s.into_text::<V>()) }
                        li() {
                            "That updates every time a new value is set on the `Proxy`"
                        }
                    }
                }
            }

            MyView {
                wrapper,
                child,
                text,
                proxy,
            }
        }
    }

    #[test]
    fn view_cast() {
        struct MyView<V: View> {
            wrapper: V::Element,
        }

        fn create_view<V: View>() -> MyView<V> {
            rsx! {
                let wrapper = div() {
                    a() {
                        "Hello"
                    }
                }
            }

            MyView { wrapper }
        }

        let view = create_view::<Ssr>();
        let cast = view.wrapper.when_element::<Web, _>(|el| Some(el.clone()));
        assert!(cast.is_none());
        let cast = view.wrapper.when_element::<Ssr, _>(|el| Some(el.clone()));
        assert!(cast.is_some());
    }

    #[test]
    #[allow(dead_code)]
    fn rsx_proxy_attribute() {
        fn view<V: View>() {
            let mut proxy = Proxy::<String>::default();

            rsx! {
                let wrapper = div() {
                    fieldset(
                        id = "blah",
                        style:display = proxy(s => s)
                    ) {}
                }
            }
        }
    }
}

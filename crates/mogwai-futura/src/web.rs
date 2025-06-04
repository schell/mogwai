//! Utilities for web (through web-sys).

use std::{borrow::Cow, cell::RefCell, ops::Deref, rc::Rc, task::Waker};

use event::EventListener;
use wasm_bindgen::{JsValue, UnwrapThrowExt, prelude::Closure};
use web_sys::wasm_bindgen::JsCast;

use crate::prelude::*;
pub mod event;

pub use mogwai_future_rsx::rsx;

pub mod prelude {
    pub use super::{Web, event::*, rsx};
    pub use crate::prelude::*;
    pub extern crate wasm_bindgen;
    pub extern crate wasm_bindgen_futures;
    pub extern crate web_sys;
}

impl ViewChild<Web> for web_sys::Node {
    fn as_append_arg(&self) -> AppendArg<Web, impl Iterator<Item = Cow<'_, web_sys::Node>>> {
        AppendArg::new(std::iter::once(Cow::Borrowed(self)))
    }
}

impl ViewParent<Web> for web_sys::Node {
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
        web_sys::Node::replace_child(self, new_node.as_ref(), old_node.as_ref()).unwrap_throw();
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

    ($ty:ident, props) => {
        node_impl!($ty);

        impl ViewProperties for web_sys::$ty {
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
    };
}

node_impl!(Text);
node_impl!(Element, props);
node_impl!(HtmlElement, props);
node_impl!(HtmlInputElement, props);

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
    type Event = web_sys::Event;

    fn next(&self) -> impl Future<Output = Self::Event> {
        self.next()
    }
}

#[derive(Clone, Copy)]
pub struct Web;

impl View for Web {
    type Element = web_sys::Element;
    type Text = web_sys::Text;
    type Node = web_sys::Node;
    type EventListener = EventListener;
}

pub struct Global<T> {
    #[cfg(target_arch = "wasm32")]
    data: std::mem::ManuallyDrop<std::cell::LazyCell<T>>,
    #[cfg(not(target_arch = "wasm32"))]
    data: std::sync::LazyLock<T>,
}

impl<T> Global<T> {
    pub const fn new(create_fn: fn() -> T) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Global {
                data: std::mem::ManuallyDrop::new(std::cell::LazyCell::new(create_fn)),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Global {
                data: std::sync::LazyLock::new(create_fn),
            }
        }
    }
}

unsafe impl<T> Send for Global<T> {}
unsafe impl<T> Sync for Global<T> {}

impl<T> Deref for Global<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

static WINDOW: Global<web_sys::Window> = Global::new(|| web_sys::window().unwrap_throw());
static DOCUMENT: Global<web_sys::Document> = Global::new(|| WINDOW.document().unwrap_throw());

/// Return the DOM [`web_sys::Window`].
/// #### Panics
/// Panics when the window cannot be returned.
pub fn window() -> &'static web_sys::Window {
    WINDOW.deref()
}

/// Return the document JsDom object [`web_sys::Document`]
/// #### Panics
/// Panics on non-wasm32 or when the document cannot be returned.
pub fn document() -> &'static web_sys::Document {
    DOCUMENT.deref()
}

/// Return the document's body Dom object.
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
pub fn next_animation_frame() -> NextFrame {
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

#[cfg(test)]
mod test {
    use crate::{self as mogwai_futura, proxy::Proxy};
    use mogwai_futura::web::prelude::*;

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
    fn rsx_block_nesting() {
        struct MyView<V: View> {
            wrapper: V::Element,
            child: MyChild<V>,
            text: V::Text,
            proxy: Proxy<V, Str>,
        }

        #[derive(ViewChild)]
        struct MyChild<V: View> {
            #[child]
            wrapper: V::Element,
        }

        fn view2<V: View>() -> MyView<V> {
            let wrapper = V::Element::new("p");
            let _wrapper_text = V::Text::new("Here lies davey jones.");
            wrapper.append_child(&_wrapper_text);
            let child = MyChild { wrapper };
            let proxy = Proxy::<V, Str>::default();
            let wrapper = V::Element::new("div");
            let child = child;
            wrapper.append_child(&child);
            let _wrapper_text = V::Text::new("Constant text can live inline.");
            wrapper.append_child(&_wrapper_text);
            let text = "But Rust expressions in a block must evaluate to some kind of node."
                .into_text::<V>();
            wrapper.append_child(&text);
            let _wrapper_ul = V::Element::new("ul");
            let __wrapper_ul_li = V::Element::new("li");
            let ___wrapper_ul_li_text =
                V::Text::new("And you can use `Proxy` to insert a variable number of nodes...");
            __wrapper_ul_li.append_child(&___wrapper_ul_li_text);
            _wrapper_ul.append_child(&__wrapper_ul_li);
            let mut __wrapper_ul_block_proxy = {
                let s = &proxy;
                mogwai_futura::proxy::ProxyChild::new(
                    &_wrapper_ul,
                    (std::ops::Deref::deref(s)).into_text::<V>(),
                )
            };
            _wrapper_ul.append_child(&__wrapper_ul_block_proxy);
            let __wrapper_ul_li1 = V::Element::new("li");
            let ___wrapper_ul_li1_text =
                V::Text::new("That updates every time a new value is set on the `Proxy`");
            __wrapper_ul_li1.append_child(&___wrapper_ul_li1_text);
            _wrapper_ul.append_child(&__wrapper_ul_li1);
            wrapper.append_child(&_wrapper_ul);
            wrapper.set_property("id", "wrapper");
            let proxy = {
                let mut proxy = proxy;
                proxy.on_update({
                    move |model| {
                        let s = model;
                        __wrapper_ul_block_proxy.replace(&_wrapper_ul, s.into_text::<V>());
                    }
                });
                proxy
            };
            MyView {
                wrapper,
                child,
                text,
                proxy,
            }
        }

        fn view<V: View>() -> MyView<V> {
            rsx! {
                let wrapper = p() {
                    "Here lies davey jones."
                }
            }

            let child = MyChild { wrapper };

            let proxy = Proxy::<V, Str>::default();

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
    fn rsx_proxy() {
        #[derive(PartialEq)]
        struct Model {
            id: usize,
            href: crate::str::Str,
            link_text: crate::str::Str,
        }

        struct MyView<V: View> {
            wrapper: V::Element,
            proxy: Proxy<V, Model>,
        }

        fn create_view<V: View>() -> MyView<V> {
            let proxy = Proxy::<V, _>::new(Model {
                id: 666,
                href: "localhost:8080".into(),
                link_text: "Go home.".into(),
            });

            rsx! {
                let wrapper = div(
                    id = proxy(m => m.id.to_string())
                ) {
                    a( href = proxy(model => &model.href) ) {
                        { proxy(model => (&model.link_text).into_text::<V>()) }
                    }
                }
            }

            MyView { wrapper, proxy }
        }

        let _view = create_view::<Web>();
    }
}

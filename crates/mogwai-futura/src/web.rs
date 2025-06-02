//! Utilities for web (through web-sys).

use std::{
    cell::{LazyCell, RefCell},
    mem::ManuallyDrop,
    ops::Deref,
    rc::Rc,
    sync::LazyLock,
    task::Waker,
};

use event::EventListener;
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

impl ViewChild<Web> for web_sys::Node {
    fn as_append_arg(&self) -> AppendArg<Web, impl Iterator<Item = &'_ web_sys::Node>> {
        AppendArg::new(std::iter::once(self))
    }
}

impl ViewParent<Web> for web_sys::Node {
    fn remove_child(&self, child: impl ViewChild<Web>) {
        for child in child.as_append_arg() {
            let _ = self.remove_child(child);
        }
    }

    fn append_child(&self, child: impl ViewChild<Web>) {
        for child in child.as_append_arg() {
            let _ = self.append_child(child);
        }
    }

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

macro_rules! node_impl {
    ($ty:ident) => {
        impl ViewEventTarget<Web> for web_sys::$ty {
            fn listen(&self, event_name: impl Into<Str>) -> EventListener {
                EventListener::new(self, event_name)
            }
        }

        impl ViewChild<Web> for web_sys::$ty {
            fn as_append_arg(&self) -> AppendArg<Web, impl Iterator<Item = &'_ web_sys::Node>> {
                let node: &web_sys::Node = self.as_ref();
                AppendArg::new(std::iter::once(node))
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

            fn remove_child(&self, child: impl ViewChild<Web>) {
                for child in child.as_append_arg() {
                    let _ = web_sys::Node::remove_child(self, &child);
                }
            }

            fn append_child(&self, child: impl ViewChild<Web>) {
                for child in child.as_append_arg() {
                    let _ = web_sys::Node::append_child(self, &child);
                }
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
    type Node<'a> = &'a web_sys::Node;
    type EventListener = EventListener;
}

pub struct Global<T> {
    #[cfg(target_arch = "wasm32")]
    data: ManuallyDrop<LazyCell<T>>,
    #[cfg(not(target_arch = "wasm32"))]
    data: LazyLock<T>,
}

impl<T> Global<T> {
    pub const fn new(create_fn: fn() -> T) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Global {
                data: ManuallyDrop::new(LazyCell::new(create_fn)),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Global {
                data: LazyLock::new(create_fn),
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
    use std::{
        marker::PhantomData,
        ops::{Deref, DerefMut},
    };

    use crate as mogwai_futura;
    use mogwai_futura::web::prelude::*;

    #[derive(Default)]
    struct Proxy<V: View, T> {
        model: T,
        update: Option<Box<dyn FnMut(&T) + 'static>>,
        _phantom: PhantomData<V>,
    }

    impl<V: View, T> Deref for Proxy<V, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.model
        }
    }

    impl<V: View, T: PartialEq> Proxy<V, T> {
        pub fn new(model: T) -> Self {
            Self {
                model,
                update: None,
                _phantom: PhantomData,
            }
        }

        pub fn on_update(&mut self, f: impl FnMut(&T) + 'static) {
            self.update = Some(Box::new(f))
        }

        pub fn set(&mut self, t: T) {
            if t != self.model {
                self.model = t;
                if let Some(update) = self.update.as_mut() {
                    update(&self.model);
                }
            }
        }
    }

    #[test]
    fn proxy() {
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
            let mut proxy = Proxy::<V, _>::new(Model {
                id: 666,
                href: "localhost:8080".into(),
                link_text: "Go home.".into(),
            });

            rsx! {
                let wrapper = div(
                    id = proxy(model) => model.id.to_string()
                ) {
                    a(
                        href = proxy(model) => &model.href
                    ) {
                        { proxy(model) => &model.link_text }
                    }
                }
            }

            // proxy.on_update({
            //     let wrapper = wrapper.clone();
            //     let link = link.clone();
            //     let link_text = link_text.clone();

            //     move |model| {
            //         wrapper.set_property("id", model.id.to_string());
            //         link.set_property("href", &model.href);
            //         link_text.set_text(&model.link_text);
            //     }
            // });

            MyView { wrapper, proxy }
        }

        let _view = create_view::<Web>();
    }
}

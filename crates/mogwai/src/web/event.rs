//! # Event Future API
//!
//! This module provides a future-based API for handling event callbacks.
//! It allows for asynchronous event handling by resolving futures when
//! events occur.
//!
//! ## Key Components
//!
//! - **EventListener**: A struct that manages event listeners for DOM elements.
//!   It registers a callback for a specific event type and provides a future
//!   that resolves when the event occurs.
//!
//! And then a low-level API for building your own listeners:
//!
//! - **Listener<T>**: A struct with a conversion function and a trigger.
//! - **Callback**: A trigger given to Javascript that resolves a future.
//!
//! ## Usage
//!
//! The `EventListener` can be used to listen for events on DOM elements. When
//! an `EventListener` is dropped, it automatically removes the associated event
//! listener from the DOM element, ensuring that no memory leaks occur and that
//! the event listener is properly cleaned up. The future resolves when the
//! event occurs, allowing for easy integration with asynchronous workflows.
//!
//! [`Listener::new`] can be used by supplying a conversion function to convert
//! [`JsValue`]s sent from Javascript into a domain type `T`. It returns the
//! [`Callback`] which can be given to Javascript code as a trigger, as well as
//! the [`Listener`], which can dole out futures with [`Listener::next`]. Those
//! futures will resolve when the [`Callback`] is triggered.
use std::{
    borrow::Cow, cell::RefCell, marker::PhantomData, ops::DerefMut, pin::Pin, rc::Rc, task::Waker,
};

use wasm_bindgen::{UnwrapThrowExt, convert::FromWasmAbi};
use wasm_bindgen_futures::wasm_bindgen::{JsCast, JsValue, prelude::Closure};

use crate::Str;

/// A trait that allows using callbacks up to arity 8 in an abstract way.
///
/// This is for internal use.
pub trait Parameters {
    type Closure: AsRef<JsValue> + 'static;

    fn into_arity_closure(f: Box<dyn FnMut(Self)>) -> Self::Closure;
}

impl Parameters for () {
    type Closure = Closure<dyn FnMut()>;

    fn into_arity_closure(mut f: Box<dyn FnMut(Self)>) -> Self::Closure {
        Closure::wrap(Box::new(move || {
            f(());
        }))
    }
}

impl<A: FromWasmAbi + 'static> Parameters for (A,) {
    type Closure = Closure<dyn FnMut(A)>;

    fn into_arity_closure(mut f: Box<dyn FnMut(Self)>) -> Self::Closure {
        Closure::wrap(Box::new(move |a| {
            f((a,));
        }))
    }
}

use crate as mogwai;
mogwai_macros::impl_parameters_tuples!((A, B));
mogwai_macros::impl_parameters_tuples!((A, B, C));
mogwai_macros::impl_parameters_tuples!((A, B, C, D));
mogwai_macros::impl_parameters_tuples!((A, B, C, D, E));
mogwai_macros::impl_parameters_tuples!((A, B, C, D, E, F));
mogwai_macros::impl_parameters_tuples!((A, B, C, D, E, F, G));
mogwai_macros::impl_parameters_tuples!((A, B, C, D, E, F, G, H));

/// A wrapper around a Rust function of arity `N` that can be triggered from
/// Javascript.
#[repr(transparent)]
pub struct Callback<Params> {
    inner: Rc<Box<dyn std::any::Any>>,
    _phantom: PhantomData<Params>,
}

impl<Params> Clone for Callback<Params> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _phantom: self._phantom,
        }
    }
}

impl<P: Parameters> Callback<P> {
    /// Return a reference to the callback as a Javascript function.
    pub fn function(&self) -> &web_sys::js_sys::Function {
        let b = self.inner.as_ref();
        let closure: &P::Closure = b
            .downcast_ref()
            .expect_throw("must construct as Parameters");
        let jsval: &JsValue = closure.as_ref();
        jsval.unchecked_ref()
    }
}

#[derive(Clone)]
struct FutureEventOccurrence<T> {
    value: Rc<RefCell<Option<T>>>,
    wakers: Rc<RefCell<Vec<Waker>>>,
}

impl<T> Default for FutureEventOccurrence<T> {
    fn default() -> Self {
        Self {
            value: Default::default(),
            wakers: Default::default(),
        }
    }
}

impl<T: Clone> std::future::Future for FutureEventOccurrence<T> {
    type Output = T;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Some(event) = self.value.borrow().as_ref() {
            std::task::Poll::Ready(event.clone())
        } else {
            // Store the waker for later.
            self.wakers.borrow_mut().push(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}

/// A generic listener that provides a [`Callback`].
///
/// The listener will "fire" an event any time the callback is called.
pub struct Listener<I, O> {
    /// The callback registered that will be invoked when the event occurs.
    callback: Rc<RefCell<Option<Callback<I>>>>,
    /// The machinery needed to notify all `.await` points that the event has
    /// occured.
    event: Rc<RefCell<FutureEventOccurrence<O>>>,
}

impl<I, O> Clone for Listener<I, O> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            event: self.event.clone(),
        }
    }
}

impl<I, O> Default for Listener<I, O> {
    fn default() -> Self {
        Self {
            callback: Default::default(),
            event: Default::default(),
        }
    }
}

impl<I: Parameters, O: Clone + 'static> Listener<I, O> {
    /// Create a new listener and callback that triggers the listener.
    ///
    /// The `convert` function takes a single parameter `I` as input, where `I`
    /// is a tuple of arity `N` values of `JsValue`, where `N` is 8 or less.
    ///
    /// The resulting callback is callable from Javascript with the same arity.
    /// So if your Javascript function takes a callback of arity 3, you
    /// would use `(JsValue, JsValue, JsValue)` as `I`.
    ///
    /// If your Javascript callback passes no parameters, `I` should be `()`.
    pub fn new(mut convert: impl FnMut(I) -> O + 'static) -> (Callback<I>, Self) {
        let event: Rc<RefCell<FutureEventOccurrence<_>>> = Default::default();
        let listener_event = event.clone();
        let convert_and_wake = Box::new(move |params: I| {
            let val = convert(params);
            // When the event happens (when this callback is called), we'll take the current
            // future event occurance, fill it out with the event, call the wakers and then
            // _drop_ it, leaving the `event` clear for the next event.
            //
            // `.await` points that are waiting for the event will have cloned the dropped
            // occurance and will receive their event by polling at the `.await` site.
            let current = std::mem::take(listener_event.borrow_mut().deref_mut());
            *current.value.borrow_mut() = Some(val);
            // Wake up all the wakers of those `.await` points
            let wakers = std::mem::take(current.wakers.borrow_mut().deref_mut());
            for waker in wakers.into_iter() {
                waker.wake();
            }
            // `current` is dropped here - now the only references to it
            // will be those in `.await` points.
            // The `std::mem::take` above replaced the
            // `FutureEventOccurrence` in `listener_event` with a fresh
            // Default...
        }) as Box<dyn FnMut(I)>;
        let closure = I::into_arity_closure(convert_and_wake);
        // let closure = Closure::wrap(convert_and_wake.wrap_args());

        let callback = Callback {
            inner: Rc::new(Box::new(closure)),
            _phantom: PhantomData::<I>,
        };
        let listener = Self {
            callback: Rc::new(RefCell::new(Some(callback.clone()))),
            event,
        };
        (callback, listener)
    }

    /// Produces a future that will resolve when the event occurs.
    ///
    /// This function can be called from multiple callsites, each receiving
    /// their own unique future that will all resolve at the next occurence.
    pub fn next(&self) -> impl std::future::Future<Output = O> {
        self.event.borrow().clone()
    }
}

/// A thin wrapper over Javascript event listeners.
#[derive(Clone)]
pub struct EventListener {
    /// The DOM that the event listener is registered upon.
    target: web_sys::EventTarget,
    /// The name of the event being listened for.
    event_name: Str,
    /// The raw listener.
    listener: Listener<(JsValue,), web_sys::Event>,
}

impl Drop for EventListener {
    fn drop(&mut self) {
        if Rc::strong_count(&self.listener.callback) == 1
            && let Some(callback) = self.listener.callback.take()
        {
            // This is the last clone of the callback, meaning this listener can be removed.
            self.target
                .remove_event_listener_with_callback(&self.event_name, callback.function())
                .unwrap();
        }
    }
}

impl EventListener {
    /// Create a new listener.
    ///
    /// This registers `event_name` on `target`.
    ///
    /// Use [`EventListener::next`] to await an event occurence.
    pub fn new(
        target: impl AsRef<web_sys::EventTarget>,
        event_name: impl Into<Cow<'static, str>>,
    ) -> Self {
        let (callback, listener) = Listener::new(|(val,): (JsValue,)| {
            // UNCHECKED: safe because this is an event callback, and events in JS are all
            // `Event`.
            let ev: web_sys::Event = val.unchecked_into();
            ev
        });

        let event_name = event_name.into();
        let target = target.as_ref().clone();
        target
            .add_event_listener_with_callback(&event_name, callback.function())
            .unwrap();

        Self {
            target,
            event_name,
            listener,
        }
    }

    /// Produces a future that will resolve when the event occurs.
    ///
    /// This function can be called from multiple callsites, each receiving
    /// their own unique future that will all resolve at the next occurence.
    pub fn next(&self) -> impl std::future::Future<Output = web_sys::Event> {
        self.listener.next()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod test {
    use super::*;
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn callback_zero_arity_resolves_future() {
        let (callback, listener) = Listener::new(|_: ()| {});

        wasm_bindgen_futures::spawn_local(async move {
            // Call the callback's JS function with no arguments.
            let f = callback.function().clone();
            f.call0(&JsValue::NULL).unwrap();
        });

        // The future should now be ready.
        let result = listener.next().await;
        assert_eq!(result, ());
    }

    /// 1-arity callback (the common case)
    #[wasm_bindgen_test]
    async fn callback_one_arity_resolves_with_value() {
        let (callback, listener) = Listener::new(|(v,): (JsValue,)| v.as_f64().unwrap_or(0.0));
        wasm_bindgen_futures::spawn_local(async move {
            let f = callback.function().clone();
            f.call1(&JsValue::NULL, &JsValue::from_f64(42.0)).unwrap();
        });
        let result = listener.next().await;
        assert_eq!(result, 42.0);
    }

    /// 2-arity callback (multi-arg, tests the tuple path)
    #[wasm_bindgen_test]
    async fn callback_two_arity_resolves_with_tuple() {
        let (callback, listener) = Listener::new(|(a, b): (JsValue, JsValue)| {
            (a.as_f64().unwrap_or(0.0), b.as_string().unwrap_or_default())
        });
        wasm_bindgen_futures::spawn_local(async move {
            let f = callback.function().clone();
            f.call2(
                &JsValue::NULL,
                &JsValue::from_f64(7.0),
                &JsValue::from_str("hello"),
            )
            .unwrap();
        });
        let (n, s) = listener.next().await;
        assert_eq!(n, 7.0);
        assert_eq!(s, "hello");
    }

    /// Multiple .next() calls all resolve on one callback invocation
    /// (fan-out)
    #[wasm_bindgen_test]
    async fn multiple_next_calls_all_resolve() {
        let (callback, listener) = Listener::new(|(v,): (JsValue,)| v.as_f64().unwrap_or(0.0));

        // Create two independent futures before firing.
        let fut1 = listener.next();
        let fut2 = listener.next();

        wasm_bindgen_futures::spawn_local(async move {
            let f = callback.function().clone();
            f.call1(&JsValue::NULL, &JsValue::from_f64(99.0)).unwrap();
        });

        // Both should resolve to the same value.
        assert_eq!(fut1.await, 99.0);
        assert_eq!(fut2.await, 99.0);
    }

    /// Sequential events — first .next() resolves, then a new .next()
    /// awaits // the next event
    #[wasm_bindgen_test]
    async fn sequential_events_resolve_in_order() {
        let (callback, listener) = Listener::new(|(v,): (JsValue,)| v.as_f64().unwrap_or(0.0));
        let f = callback.function();

        wasm_bindgen_futures::spawn_local({
            let f = f.clone();
            async move {
                f.call1(&JsValue::NULL, &JsValue::from_f64(1.0)).unwrap();
            }
        });
        assert_eq!(listener.next().await, 1.0);

        wasm_bindgen_futures::spawn_local({
            let f = f.clone();
            async move {
                f.call1(&JsValue::NULL, &JsValue::from_f64(2.0)).unwrap();
            }
        });
        assert_eq!(listener.next().await, 2.0);
    }

    /// EventListener still works (regression test for the existing
    /// DOM path)
    #[wasm_bindgen_test]
    async fn event_listener_resolves_on_dom_event() {
        use web_sys::HtmlElement;
        let el = mogwai::web::document()
            .create_element("button")
            .unwrap()
            .dyn_into::<HtmlElement>()
            .unwrap();
        let listener = EventListener::new(&el, "click");

        wasm_bindgen_futures::spawn_local(async move {
            // Dispatch a synthetic click.
            let event = web_sys::Event::new("click").unwrap();
            el.dispatch_event(&event).unwrap();
        });

        let ev = listener.next().await;
        assert_eq!(ev.type_(), "click");
    }
}

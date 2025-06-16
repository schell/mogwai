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
//! ## Usage
//!
//! The `EventListener` can be used to listen for events on DOM elements. When
//! an `EventListener` is dropped, it automatically removes the associated event
//! listener from the DOM element, ensuring that no memory leaks occur and that
//! the event listener is properly cleaned up. The future resolves when the event
//! occurs, allowing for easy integration with asynchronous workflows.
use std::{borrow::Cow, cell::RefCell, ops::DerefMut, pin::Pin, rc::Rc, task::Waker};

use wasm_bindgen_futures::wasm_bindgen::{JsCast, JsValue, prelude::Closure};

use crate::Str;

type Callback = Rc<Closure<dyn FnMut(JsValue)>>;

#[derive(Clone, Default)]
struct FutureEventOccurrence {
    event: Rc<RefCell<Option<web_sys::Event>>>,
    wakers: Rc<RefCell<Vec<Waker>>>,
}

impl std::future::Future for FutureEventOccurrence {
    type Output = web_sys::Event;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Some(event) = self.event.borrow().as_ref() {
            std::task::Poll::Ready(event.clone())
        } else {
            // Store the waker for later.
            self.wakers.borrow_mut().push(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}

/// A thin wrapper over Javascript event listeners.
#[derive(Clone)]
pub struct EventListener {
    /// The DOM that the event listener is registered upon.
    target: web_sys::EventTarget,
    /// The name of the event being listened for.
    event_name: Str,
    /// The callback registered that will be invoked when the event occurs.
    callback: Rc<RefCell<Option<Callback>>>,
    /// The machinery needed to notify all `.await` points that the event has occured.
    events: Rc<RefCell<FutureEventOccurrence>>,
}

impl Drop for EventListener {
    fn drop(&mut self) {
        if Rc::strong_count(&self.callback) == 1 {
            if let Some(rc_callback) = self.callback.take() {
                if let Ok(callback) = Rc::try_unwrap(rc_callback) {
                    // This is the last clone of the callback, meaning this listener can be removed.
                    self.target
                        .remove_event_listener_with_callback(
                            &self.event_name,
                            callback.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                }
            }
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
        let events: Rc<RefCell<FutureEventOccurrence>> = Default::default();
        let callback = Closure::wrap({
            let events = events.clone();
            Box::new(move |val: JsValue| {
                // UNCHECKED: safe because this is an event callback, and events in JS are all `Event`.
                let ev: web_sys::Event = val.unchecked_into();
                // When the event happens (when this callback is called), we'll take the current
                // future event occurance, fill it out with the event, call the wakers and then
                // _drop_ it, leaving the `events` clear for the next event.
                //
                // `.await` points that are waiting for the event will have cloned the dropped
                // occurance and will receive their event by polling at the `.await` site.
                let event = std::mem::take(events.borrow_mut().deref_mut());
                *event.event.borrow_mut() = Some(ev);
                // Wake up all the wakers of those `.await` points
                let wakers = std::mem::take(event.wakers.borrow_mut().deref_mut());
                for waker in wakers.into_iter() {
                    waker.wake();
                }
            }) as Box<dyn FnMut(JsValue)>
        });

        let event_name = event_name.into();
        let target = target.as_ref().clone();
        target
            .add_event_listener_with_callback(&event_name, callback.as_ref().unchecked_ref())
            .unwrap();

        Self {
            target,
            event_name,
            callback: Rc::new(RefCell::new(Some(Rc::new(callback)))),
            events,
        }
    }

    /// Produces a future that will resolve when the event occurs.
    ///
    /// This function can be called from multiple callsites, each receiving their own
    /// unique future that will all resolve at the next occurence.
    pub fn next(&self) -> impl std::future::Future<Output = web_sys::Event> {
        self.events.borrow().clone()
    }
}

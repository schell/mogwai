//! A stream API for event callbacks.
//!
//! This uses [`futures-lite::Stream`] to send events to downstream listeners.
use std::{cell::RefCell, ops::DerefMut, pin::Pin, rc::Rc, task::Waker};

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
            log::trace!("event proc'd");
            std::task::Poll::Ready(event.clone())
        } else {
            // Store the waker for later.
            self.wakers.borrow_mut().push(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}

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
                            self.event_name.as_str(),
                            callback.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                    log::trace!(
                        "dropping listener for {} on target {:?}",
                        self.event_name,
                        self.target
                    );
                }
            }
        }
    }
}

impl EventListener {
    pub fn new(target: impl AsRef<web_sys::EventTarget>, event_name: impl Into<Str>) -> Self {
        let event_name = event_name.into();
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

        let target = target.as_ref().clone();
        target
            .add_event_listener_with_callback(
                event_name.as_str(),
                callback.as_ref().unchecked_ref(),
            )
            .unwrap();

        Self {
            target,
            event_name,
            callback: Rc::new(RefCell::new(Some(Rc::new(callback)))),
            events,
        }
    }

    pub fn next(&self) -> impl std::future::Future<Output = web_sys::Event> {
        self.events.borrow().clone()
    }
}

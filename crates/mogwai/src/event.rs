//! View events as streams of values.
//!
//! Events in Mogwai are registered and sent down a stream to be
//! consumed by logic loops. When an event stream
//! is dropped, its resources are cleaned up automatically.
use futures::{Sink, SinkExt, Stream, StreamExt};
use log::info;
use std::{cell::RefCell, pin::Pin, rc::Rc, task::Waker};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::EventTarget;

use crate::futures::SinkError;

struct WebCallback {
    target: EventTarget,
    name: String,
    closure: Option<Closure<dyn FnMut(JsValue)>>,
    waker: Rc<RefCell<Option<Waker>>>,
    event: Rc<RefCell<Option<web_sys::Event>>>,
}

impl Drop for WebCallback {
    fn drop(&mut self) {
        if let Some(closure) = self.closure.take() {
            self.target
                .remove_event_listener_with_callback(
                    self.name.as_str(),
                    closure.as_ref().unchecked_ref(),
                )
                .unwrap();
        }
    }
}

impl Stream for WebCallback {
    type Item = web_sys::Event;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        log::info!("polling {}", self.name);
        let data = self.get_mut();
        data.waker.replace(Some(cx.waker().clone()));

        if let Some(event) = data.event.borrow_mut().take() {
            std::task::Poll::Ready(Some(event))
        } else {
            std::task::Poll::Pending
        }
    }
}

/// Listen for events of the given name on the given target.
/// All events will be sent downstream until the stream is
/// dropped.
pub fn event_stream(
    ev_name: &str,
    target: &web_sys::EventTarget,
) -> impl Stream<Item = web_sys::Event> {
    let waker: Rc<RefCell<Option<Waker>>> = Default::default();
    let waker_here = waker.clone();

    let event: Rc<RefCell<Option<web_sys::Event>>> = Default::default();
    let event_here = event.clone();

    let closure = Closure::wrap(Box::new(move |val: JsValue| {
        let ev = val.unchecked_into();
        event.replace(Some(ev));
        if let Some(waker) = waker.borrow_mut().take() {
            waker.wake()
        }
    }) as Box<dyn FnMut(JsValue)>);

    target
        .add_event_listener_with_callback(ev_name, closure.as_ref().unchecked_ref())
        .unwrap();

    WebCallback {
        target: target.clone(),
        name: ev_name.to_string(),
        closure: Some(closure),
        event: event_here,
        waker: waker_here,
    }
}

/// Add an event listener of the given name to the given target. When the event happens, the
/// event will be fed to the given sink. If the sink is closed, the listener will be removed.
pub fn add_event(
    ev_name: &str,
    target: &web_sys::EventTarget,
    mut tx: Pin<Box<dyn Sink<web_sys::Event, Error = SinkError> + 'static>>,
) {
    log::info!("adding event {}", ev_name);
    let mut stream = event_stream(ev_name, target);
    let ev_name = ev_name.to_string();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            log::info!("event loop");
            match stream.next().await {
                Some(event) => {
                    info!("'{}' got event {:?}", ev_name, event);
                    match tx.send(event).await {
                        Ok(()) => {
                            info!("sent");
                        }
                        Err(SinkError::Full) => panic!("event sink is full"),
                        Err(SinkError::Closed) => {
                            info!("closed, breaking");
                            break;
                        }
                    }
                }
                None => {
                    info!("event {} stream ended, breaking", ev_name);
                    break;
                }
            }
        }
    });
}

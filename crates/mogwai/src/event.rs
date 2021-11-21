//! View events as streams of values.
//!
//! Events in Mogwai are registered and sent down a stream to be
//! consumed by logic loops. When an event stream
//! is dropped, its resources are cleaned up automatically.
use futures::{Sink, SinkExt, Stream, StreamExt};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::Waker,
};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::EventTarget;

use crate::{
    futures::SinkError,
    target::{Sendable, Streamable},
};

struct WebCallback {
    target: EventTarget,
    name: String,
    closure: Option<Closure<dyn FnMut(JsValue)>>,
    waker: Arc<Mutex<Option<Waker>>>,
    event: Arc<Mutex<Option<web_sys::Event>>>,
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
            log::trace!("dropping event {}", self.name);
        }
    }
}

impl Stream for WebCallback {
    type Item = web_sys::Event;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let data = self.get_mut();
        *data.waker.lock().unwrap() = Some(cx.waker().clone());

        if let Some(event) = data.event.lock().unwrap().take() {
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
    let waker: Arc<Mutex<Option<Waker>>> = Default::default();
    let waker_here = waker.clone();

    let event: Arc<Mutex<Option<web_sys::Event>>> = Default::default();
    let event_here = event.clone();

    let closure = Closure::wrap(Box::new(move |val: JsValue| {
        let ev = val.unchecked_into();
        *event.lock().unwrap() = Some(ev);
        if let Some(waker) = waker.lock().unwrap().take() {
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

/// Listen for events of the given name on the given target.
/// Run the event through the given function and sent the result on the given sink.
///
/// This can be used to get a `Sendable` stream of events from a `web_sys::EventTarget`.
pub fn event_stream_with<T: Sendable>(
    ev_name: &str,
    target: &web_sys::EventTarget,
    mut f: impl FnMut(web_sys::Event) -> T + 'static,
) -> impl Streamable<T> {
    let (mut tx, rx) = futures::channel::mpsc::unbounded();
    let mut stream = event_stream(ev_name, target);
    wasm_bindgen_futures::spawn_local(async move {
        while let Some(msg) = stream.next().await {
            let t = f(msg);
            match tx.send(t).await.ok() {
                Some(()) => {}
                None => break,
            }
        }
    });

    rx
}

/// Add an event listener of the given name to the given target. When the event happens, the
/// event will be fed to the given sink. If the sink is closed, the listener will be removed.
pub fn add_event(
    ev_name: &str,
    target: &web_sys::EventTarget,
    mut tx: Pin<Box<dyn Sink<web_sys::Event, Error = SinkError> + 'static>>,
) {
    let mut stream = event_stream(ev_name, target);
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            match stream.next().await {
                Some(event) => match tx.send(event).await {
                    Ok(()) => {}
                    Err(SinkError::Full) => panic!("event sink is full"),
                    Err(SinkError::Closed) => break,
                },
                None => break,
            }
        }
    });
}

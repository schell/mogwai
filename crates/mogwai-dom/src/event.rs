//! View events as streams of values.
//!
//! Events in Mogwai are registered and sent down a stream to be
//! consumed by logic loops. When an event stream
//! is dropped, its resources are cleaned up automatically.
use mogwai::{
    channel::broadcast,
    sink::{Sink, TrySendError},
    stream::{Stream, StreamExt},
};
use send_wrapper::SendWrapper;
use std::{
    pin::Pin,
    sync::Arc,
};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};

use crate::view::js::JsDom;

/// A wrapper for [`web_sys::Event`].
#[derive(Clone, Debug)]
pub struct JsDomEvent {
    inner: SendWrapper<Arc<JsValue>>,
}

//impl TryFrom<serde_json::Value> for JsDomEvent {
//    type Error = serde_json::Error;
//
//    #[cfg(target_arch = "wasm32")]
//    fn try_from(value: serde_json::Value) -> serde_json::Result<Self> {
//        let inner: JsValue = JsValue::from_serde(&value)?;
//        Ok(JsDomEvent { inner })
//    }
//    #[cfg(not(target_arch = "wasm32"))]
//    fn try_from(value: serde_json::Value) -> serde_json::Result<Self> {
//        Ok(JsDomEvent { inner: value })
//    }
//}

impl From<web_sys::Event> for JsDomEvent {
    fn from(ev: web_sys::Event) -> Self {
        let inner = SendWrapper::new(Arc::new(JsValue::from(ev)));
        JsDomEvent { inner }
    }
}

impl From<&web_sys::Event> for JsDomEvent {
    fn from(ev: &web_sys::Event) -> Self {
        let inner = SendWrapper::new(Arc::new(JsValue::from(ev)));
        JsDomEvent { inner }
    }
}

impl JsDomEvent {
    /// Attempt to convert into a `web_sys::Event`. This only works when running on wasm32.
    pub fn browser_event(self) -> Option<web_sys::Event> {
        self.inner.dyn_ref::<web_sys::Event>().cloned()
    }

    pub fn clone_as<T: JsCast + Clone>(&self) -> Option<T> {
        self.inner.dyn_ref::<T>().cloned()
    }
}

pub(crate) struct WebCallback {
    target: JsDom,
    name: &'static str,
    closure: Option<SendWrapper<Closure<dyn FnMut(JsValue)>>>,
}

impl Drop for WebCallback {
    fn drop(&mut self) {
        if let Some(closure) = self.closure.take() {
            let target = self.target.clone_as::<web_sys::EventTarget>().unwrap();
            target
                .remove_event_listener_with_callback(
                    self.name,
                    closure.as_ref().unchecked_ref(),
                )
                .unwrap();
        }
    }
}

/// Add an event listener of the given name to the given target. When the event happens, the
/// event will be fed to the given sink. If the sink is closed, the listener will be removed.
pub(crate) fn add_event(
    ev_name: &'static str,
    target: &web_sys::EventTarget,
    tx: Pin<Box<dyn Sink<JsDomEvent> + Send + Sync + 'static>>,
) -> WebCallback {
    let closure = Closure::wrap(Box::new(move |val: JsValue| {
        let ev: web_sys::Event = val.unchecked_into();
        let js_dom_event = JsDomEvent::from(&ev);
        match tx.try_send(js_dom_event) {
            Ok(()) => {}
            Err(TrySendError::Busy) => {
                log::error!("channel for event {:?} is busy", ev);
            }
            Err(TrySendError::Closed) => {}
            Err(TrySendError::Full) => {
                log::error!("channel for event {:?} is full", ev);
            }
        }
    }) as Box<dyn FnMut(JsValue)>);

    target
        .add_event_listener_with_callback(ev_name, closure.as_ref().unchecked_ref())
        .unwrap();

    WebCallback {
        target: JsDom::from_jscast(target),
        name: ev_name,
        closure: Some(SendWrapper::new(closure)),
    }
}

/// Listen for events of the given name on the given target.
/// All events will be sent downstream until the stream is
/// dropped.
pub fn event_stream(
    ev_name: &'static str,
    target: &web_sys::EventTarget,
) -> impl Stream<Item = JsDomEvent> + Send {
    let (tx, rx) = broadcast::bounded(1);
    let callback = add_event(ev_name, target, Box::pin(tx));

    #[allow(dead_code)]
    struct EventStream {
        callback: WebCallback,
        rx: broadcast::Receiver<JsDomEvent>,
    }

    impl Stream for EventStream {
        type Item = JsDomEvent;

        fn poll_next(
            self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            self.get_mut().rx.poll_next(cx)
        }
    }

    EventStream { callback, rx }
}

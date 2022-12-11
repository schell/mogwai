//! View events as streams of values.
//!
//! Events in Mogwai are registered and sent down a stream to be
//! consumed by logic loops. When an event stream
//! is dropped, its resources are cleaned up automatically.
use futures::{Sink, SinkExt, Stream, StreamExt};
use send_wrapper::SendWrapper;
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::Waker,
};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use mogwai::channel::SinkError;

use crate::prelude::{FutureTask, JsDom};

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

impl JsDomEvent {
    ///// Return the inner event as `JsValue` on wasm32 or `serde_json::Value` on
    ///// other targets.
    //pub fn clone_inner(&self) -> Either<JsValue, serde_json::Value> {
    //    Either::Left(self.inner.clone())
    //}
    //#[cfg(not(target_arch = "wasm32"))]
    ///// Return the inner event as `JsValue` on wasm32 or `serde_json::Value` on
    ///// other targets.
    //pub fn clone_inner(&self) -> Either<JsValue, serde_json::Value> {
    //    Either::Right(self.inner.clone())
    //}

    ///// Use `T`'s `DeserializeOwned` implementation to convert into `T`.
    //pub fn try_deserialize<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
    //    //match self.clone_inner() {
    //    self.inner.clone().into_serde()
    //    //    Either::Right(value) => serde_json::from_value(value),
    //    //}
    //}

    /// Attempt to convert into a `web_sys::Event`. This only works when running on wasm32.
    pub fn browser_event(self) -> Option<web_sys::Event> {
        self.inner.dyn_ref::<web_sys::Event>().cloned()
    }

    pub fn clone_as<T: JsCast + Clone>(&self) -> Option<T> {
        self.inner.dyn_ref::<T>().cloned()
    }
}

struct WebCallback {
    target: JsDom,
    name: String,
    closure: Option<SendWrapper<Closure<dyn FnMut(JsValue)>>>,
    waker: Arc<Mutex<Option<Waker>>>,
    event: Arc<Mutex<Option<JsDomEvent>>>,
}

impl Drop for WebCallback {
    fn drop(&mut self) {
        if let Some(closure) = self.closure.take() {
            let target = self.target.clone_as::<web_sys::EventTarget>().unwrap();
            target
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
    type Item = JsDomEvent;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let data = self.get_mut();
        *data.waker.lock().unwrap() = Some(cx.waker().clone());

        if let Some(event) = data.event.lock().unwrap().take() {
            std::task::Poll::Ready(Some(JsDomEvent::from(event)))
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
) -> impl Stream<Item = JsDomEvent> + Send {
    let waker: Arc<Mutex<Option<Waker>>> = Default::default();
    let waker_here = waker.clone();

    let event: Arc<Mutex<Option<JsDomEvent>>> = Default::default();
    let event_here = event.clone();

    let closure = Closure::wrap(Box::new(move |val: JsValue| {
        let ev: web_sys::Event = val.unchecked_into();
        *event.lock().unwrap() = Some(JsDomEvent::from(ev));
        if let Some(waker) = waker.lock().unwrap().take() {
            waker.wake()
        }
    }) as Box<dyn FnMut(JsValue)>);

    target
        .add_event_listener_with_callback(ev_name, closure.as_ref().unchecked_ref())
        .unwrap();

    WebCallback {
        target: JsDom::from_jscast(target),
        name: ev_name.to_string(),
        closure: Some(SendWrapper::new(closure)),
        event: event_here,
        waker: waker_here,
    }
}

///// Listen for events of the given name on the given target.
///// Run the event through the given function and send the result on the given sink.
/////
///// This can be used to get a `Sendable` stream of events from a `web_sys::EventTarget`.
//pub fn event_stream_with<T, V>(
//    ev_name: &str,
//    target: &web_sys::EventTarget,
//    mut f: impl FnMut(web_sys::Event) -> T + 'static,
//) -> impl Stream<Item = T> + Send + Sync + 'static
//where
//    T: Send + Sync + 'static,
//    V: View,
//{
//    let (mut tx, rx) = futures::channel::mpsc::unbounded();
//    let mut stream = event_stream(ev_name, target);
//    wasm_bindgen_futures::spawn_local(async move {
//        while let Some(msg) = stream.next().await {
//            let t = f(msg);
//            match tx.send(t).await.ok() {
//                Some(()) => {}
//                None => break,
//            }
//        }
//    });
//
//    rx
//}

/// Add an event listener of the given name to the given target. When the event happens, the
/// event will be fed to the given sink. If the sink is closed, the listener will be removed.
pub(crate) fn add_event(
    id_string: &str,
    node_id: usize,
    ev_name: &str,
    target: &web_sys::EventTarget,
    mut tx: Pin<Box<dyn Sink<JsDomEvent, Error = SinkError> + Send + 'static>>,
) -> FutureTask<()> {
    let mut stream = event_stream(ev_name, target);
    let ev_name = ev_name.to_string();
    let target = JsDom::from_jscast(target);
    FutureTask {
        name: format!("event_{}_{}_{}", id_string, node_id, ev_name),
        fut: Box::pin(async move {
            loop {
                match stream.next().await {
                    Some(event) => match tx.send(event).await {
                        Ok(()) => {}
                        Err(SinkError::Full) => panic!("event sink is full"),
                        Err(SinkError::Closed) => break,
                    },
                    None => {
                        log::trace!(
                            "removing event '{}' from {:?}",
                            ev_name,
                            target.clone_as::<web_sys::Node>()
                        );
                        break;
                    }
                }
            }
        }),
    }
}

use futures::{Sink, SinkExt, Stream, StreamExt};
use log::info;
use std::{cell::RefCell, pin::Pin, rc::Rc, task::Waker};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::EventTarget;

use crate::channel::SinkError;

struct WebCallback {
    target: EventTarget,
    name: String,
    closure: Option<Closure<dyn FnMut(JsValue)>>,
    waker: Rc<RefCell<Option<Waker>>>,
    event: Rc<RefCell<Option<web_sys::Event>>>,
}

impl Drop for WebCallback {
    fn drop(&mut self) {
        log::info!("dropping {}", self.name);
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

        if data.closure.is_none() {
            info!("creating closure for {}", data.name);
            // We haven't added the event listener yet, add it and populate the waker
            let waker = data.waker.clone();
            let event = data.event.clone();
            let closure = Closure::wrap(Box::new(move |val: JsValue| {
                log::info!("fired");
                let ev = val.unchecked_into();
                event.replace(Some(ev));
                waker.borrow_mut().take().unwrap().wake();
            }) as Box<dyn FnMut(JsValue)>);
            data.target
                .add_event_listener_with_callback(&data.name, closure.as_ref().unchecked_ref())
                .unwrap();
            data.closure = Some(closure);
        }

        if let Some(event) = data.event.borrow_mut().take() {
            std::task::Poll::Ready(Some(event))
        } else {
            std::task::Poll::Pending
        }
    }
}

pub fn event_stream(
    ev_name: &str,
    target: &web_sys::EventTarget,
) -> impl Stream<Item = web_sys::Event> {
    WebCallback {
        target: target.clone(),
        name: ev_name.to_string(),
        closure: None,
        event: Default::default(),
        waker: Default::default(),
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

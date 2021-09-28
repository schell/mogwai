use std::sync::{Arc, Mutex};
use async_channel::{Sender, TrySendError};
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use web_sys::EventTarget;

pub struct WebCallback {
    pub closure: Closure<dyn FnMut(JsValue)>,
    pub target: EventTarget,
    pub name: String,
}

impl WebCallback {
    fn cleanup(&self) {
        self.target
            .remove_event_listener_with_callback(
                self.name.as_str(),
                self.closure.as_ref().unchecked_ref(),
            )
            .unwrap();
    }
}

impl Drop for WebCallback {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Add an event listener of the given name to the given target. When the event happens, the
/// event will be fed to the given mutable function and the result will be sent on the given
/// sender. If the sender is closed, the listener will be removed.
pub fn add_event<T, F>(ev_name: &str, target: &web_sys::EventTarget, tx: Sender<T>, mut f: F)
where
    T: 'static,
    F: FnMut(web_sys::Event) -> T + 'static,
{
    let cb: Arc<Mutex<Option<WebCallback>>> = Arc::new(Mutex::new(None));
    let cb_here = cb.clone();
    let closure = Closure::wrap(Box::new(move |val: JsValue| {
        let ev = val.unchecked_into();
        match tx.try_send(f(ev)) {
            Ok(_) => {}
            Err(err) => match err {
                TrySendError::Full(_) => panic!("event handler Sender is full"),
                TrySendError::Closed(_) => {
                    // take the WebCallback, cleaning up and removing the listener
                    let _ = cb_here.lock().unwrap().take();
                }
            },
        }
    }) as Box<dyn FnMut(JsValue)>);

    target
        .add_event_listener_with_callback(ev_name, closure.as_ref().unchecked_ref())
        .unwrap();

    *cb.lock().unwrap() = Some(WebCallback {
        closure,
        target: target.clone(),
        name: ev_name.to_string(),
    });
}

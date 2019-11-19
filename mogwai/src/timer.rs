pub use web_sys::{Request, RequestInit, RequestMode, Response, XmlHttpRequest};
use wasm_bindgen::closure::Closure;
pub use wasm_bindgen_futures::{JsFuture, future_to_promise};
use wasm_bindgen::{JsCast, JsValue};
use js_sys::Promise;
use futures::Future;
use std::sync::{Arc, Mutex};

use super::txrx::*;
use super::gizmo::window;


 /// Creates a transmitter/receiver pair for an http request.
 /// The request is executed as a response to a message on the transmitter.
 /// The response is sent as a message on the receiver.
 /// Requests that come in on the transmitter while another request is in
 /// flight will be ignored.
pub fn timer_terminals(interval: u32) -> (Transmitter<XmlHttpRequest>, Receiver<Response>) {
  let req_tx = terminals::<XmlHttpRequest>();
  let (resp_tx, resp_rx) = terminals::<Response>();

  let may_promise:Arc<Mutex<Option<Promise>>> =
    Arc::new(Mutex::new(None));

  req_rx.set_responder(move |req| {
    let is_free =
      may_promise
      .try_lock()
      .expect("Could not try_lock request_terminals::set_responder")
      .is_none();

    if is_free {
      let request_promise:Promise =
        window()
        .fetch_with_request(&req);

      let future =
        JsFuture::from(request_promise)
        .and_then(|resp_value:JsValue| {
          trace!("Got a response from a request");
          let resp =
            resp_value
            .clone()
            .dyn_into()
            .expect("Result of request is not a Response");
          resp_tx.send(&resp);
          *may_promise
            .try_lock()
            .expect("Could not try_lock request_terminals::set_responder in future completion")
            = None;
          Ok(resp_value)
        });

      let promise:Promise =
        future_to_promise(future);
      *may_promise
        .try_lock()
        .expect("Could not try_lock request_terminals::set_responder set future")
        = Some(promise);
    } else {
      warn!("mogwai::request::request_terminals throttling requests - received a request while another was in flight");
    }
  });
  (req_tx, resp_rx)
}

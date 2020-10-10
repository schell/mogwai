//! Async/await for DOM events.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::sync::{Arc, Mutex};
use web_sys::{Event, EventTarget};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use mogwai::utils::window;


#[derive(Clone)]
pub struct EventResult {
  pub event: Event,
  pub elapsed: f64
}


struct EventFuture {
  event: Arc<Mutex<Option<Event>>>,
  waker: Arc<Mutex<Option<Waker>>>,
  start: f64
}


impl EventFuture {
  pub fn new(name: &str, target: &EventTarget) -> Self {
    let now =
      window()
      .performance()
      .expect("no performance object")
      .now();
    // Let most of the fields be filled out later...
    let waker = Arc::new(Mutex::new(None));
    let waker_var = waker.clone();
    let event = Arc::new(Mutex::new(None));
    let cb_event = event.clone();
    let cb:Arc<Mutex<Option<Closure<dyn FnMut(JsValue)>>>> = Arc::new(Mutex::new(None));
    let cb_var = cb.clone();
    let cb_target = target.clone();
    let cb_event_name = name.to_string();
    let closure =
      Closure::wrap(Box::new(move |val:JsValue| {
        // The event proc'd!
        // Store the event...
        let mut event_var =
          cb_event
          .try_lock()
          .expect("could not acquire lock on EventFuture::poll event var");
        let event =
          val
          .dyn_into::<Event>()
          .expect("could not cast event in EventFuture::poll");
        *event_var = Some(event);

        // Remove the event listener
        let listener =
          cb_var
          .try_lock()
          .expect("could not acquire lock on EventFuture::poll cb var")
          .take()
          .expect("no listener callback");
        cb_target
          .remove_event_listener_with_callback(
            &cb_event_name,
            listener.as_ref().unchecked_ref(),
          )
          .expect("could not remove listener");

        // wake the waker...
        let mut waker_var =
          waker_var
          .try_lock()
          .expect("could not acquire lock on EventFuture waker");
        let waker:Waker =
          waker_var
          .take()
          .expect("could not unwrap stored waker on ElementFuture");
        waker.wake();
      }) as Box<dyn FnMut(JsValue)>);

    // Now that we've created the callback, add it as a listener...
    target.add_event_listener_with_callback(
      name,
      closure.as_ref().unchecked_ref()
    ).expect("could not add listener");

    // ...and store it
    let mut cb_var =
      cb
      .try_lock()
      .expect("could not acquire lock on future.cb");
    *cb_var = Some(closure);

    EventFuture {
      event,
      waker,
      start: now
    }
  }
}


impl Future for EventFuture {
  type Output = EventResult;

  fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
    let future = self.get_mut();

    // Either the callback proc'd or something woke us up
    let may_event =
      future
      .event
      .try_lock()
      .expect("could not acquire lock on EventFuture event")
      .take();
    if let Some(event) = may_event {
      let now =
        window()
        .performance()
        .expect("no performance object")
        .now();

      Poll::Ready(EventResult {
        elapsed: now - future.start,
        event: event
      })
    } else {
      let mut waker_var =
        future
        .waker
        .try_lock()
        .expect("could not acquire lock on waker");
      *waker_var = Some(ctx.waker().clone());
      Poll::Pending
    }
  }
}


pub async fn wait_for_event_on(name: &str, target: &EventTarget) -> EventResult {
  EventFuture::new(name, target).await
}

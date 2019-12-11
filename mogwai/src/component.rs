//! Elmesque components through model and view message passing.
//!
//! Sometimes an application can get so entangled that it's hard to follow the
//! path of messages through `Transmitter`s, `Receiver`s and fold functions. For
//! situations like these where complexity is unavoidable, Mogwai provides the
//! [`Component`] trait and the helper struct [`GizmoComponent`].
//!
//! Many rust web app libraries use a message passing pattern made famous by
//! the Elm architecture to wrangle complexity. Mogwai is similar, but different
//! - Like other libraries, messages come out of the DOM into your component's model by way of the [Component::update] function.
//! - The model is updated according to the value of the model message.
//! - _Unlike_ Elm-like libraries, view updates are sent out of the update
//!   function by hand! This sounds tedious but it's actually no big deal. You'll
//!   soon understand how easy this is in practice.
//!
//! Mogwai lacks a virtual DOM implementation. One might think that this is a
//! disadvantage but to the contrary this is a strength, as it obviates the
//! entire diffing phase of rendering DOM. This is where Mogwai gets its speed
//! advantage.
//!
//! Instead of a virtual DOM Mogwai uses one more step in its model update. The
//! `Component::update` method is given a `Transmitter<Self::ViewMsg>` with which
//! to send _view update messages_. Messages sent on this transmitter will in
//! turn be sent out to the view to update the DOM. This forms a cycle. Messages
//! come into the model from the view, update, messages go into the view from the
//! model. In this way DOM updates are obvious. You know exactly where, when and
//! why updates are made (both to the model and the view).
//!
//! Here is a minimal example of a `Component` that counts its own clicks.
//!
//! ```rust, no_run
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! enum In {
//!   Click
//! }
//!
//! #[derive(Clone)]
//! enum Out {
//!   DrawClicks(i32)
//! }
//!
//! struct App {
//!   num_clicks: i32
//! }
//!
//! impl Component for App {
//!   type ModelMsg = In;
//!   type ViewMsg = Out;
//!
//!   fn builder(&self, tx: Transmitter<In>, rx:Receiver<Out>) -> GizmoBuilder {
//!     button()
//!       .tx_on("click", tx.contra_map(|_| In::Click))
//!       .rx_text("clicks = 0", rx.branch_map(|msg| {
//!         match msg {
//!           Out::DrawClicks(n) => {
//!             format!("clicks = {}", n)
//!           }
//!         }
//!       }))
//!   }
//!
//!   fn update(&mut self, msg: &In, tx_view: &Transmitter<Out>, _sub: &Subscriber<In>) {
//!     match msg {
//!       In::Click => {
//!         self.num_clicks += 1;
//!         tx_view.send(&Out::DrawClicks(self.num_clicks));
//!       }
//!     }
//!   }
//! }
//!
//!
//! pub fn main() -> Result<(), JsValue> {
//!   App{ num_clicks: 0 }
//!   .into_component()
//!   .run()
//! }
//! ```
//!
//! The first step is to define the incoming messages that will update the model.
//! Next we define the outgoing messages that will update our view. The `builder`
//! trait method uses these message types to build the view. It does this by
//! consuming a `Transmitter<Self::ModelMsg>` and a `Receiver<Self::ViewMsg>`.
//! These represent the inputs and the outputs of your component. Roughly,
//! `Self::ModelMsg` comes into the `update` function and `Self::ViewMsg`s go out
//! of the `update` function.
//!
//! ## Communicating to components
//!
//! If your component is owned by another, the parent component can communicate to
//! the child through its messages, either by calling [`GizmoComponent::update`]
//! on the child component within its own `update` function or by subscribing to
//! the child component's messages when the child component is created (see
//! [`Subscriber`]).
//!
//! ## Placing components
//!
//! Components may be used within a [`GizmoBuilder`] using the
//! [`GizmoBuilder::with_component`] function.
use std::sync::{Arc, Mutex};
use std::any::Any;
use web_sys::HtmlElement;
use wasm_bindgen::JsValue;

use super::txrx::{Transmitter, Receiver, txrx};
use super::builder::GizmoBuilder;
use super::gizmo::Gizmo;
use super::utils;

pub mod subscriber;
use subscriber::Subscriber;


/// Defines a component with distinct input (model update) and output
/// (view update) messages.
///
/// See the [module level documentation](super::component) for more details.
pub trait Component
where
  Self: Any + Sized,
  Self::ModelMsg: Clone,
  Self::ViewMsg: Clone,
{
  /// A model message comes out from the view through a tx_on function into your
  /// component's update function.
  type ModelMsg;

  /// A view message comes out from your component's update function and changes
  /// the view by being used in an rx_* function.
  type ViewMsg;

  /// Update this component in response to any received model messages.
  /// This is essentially the component's fold function.
  fn update(
    &mut self,
    msg: &Self::ModelMsg,
    tx_view: &Transmitter<Self::ViewMsg>,
    sub: &Subscriber<Self::ModelMsg>
  );



  /// Produce this component's gizmo builder using inputs and outputs.
  fn builder(
    &self,
    tx: Transmitter<Self::ModelMsg>,
    rx: Receiver<Self::ViewMsg>
  ) -> GizmoBuilder;

  /// Helper function for constructing a GizmoComponent for a type that
  /// implements Component.
  fn into_component(self) -> GizmoComponent<Self> {
    GizmoComponent::new(self)
  }
}


impl<T:Component> From<T> for GizmoBuilder {
  fn from(component: T) -> GizmoBuilder {
    let gizmo_component = component.into_component();
    let builder = gizmo_component.builder.unwrap();
    builder
  }
}


/// A component and all of its pieces.
pub struct GizmoComponent<T:Component> {
  pub trns: Transmitter<T::ModelMsg>,
  pub recv: Receiver<T::ViewMsg>,
  pub builder: Option<GizmoBuilder>,
  pub gizmo: Option<Gizmo>,
  pub state: Arc<Mutex<T>>
}


impl<T> GizmoComponent<T>
where
  T: Component + 'static,
  T::ViewMsg: Clone
{
  pub fn new(init: T) -> GizmoComponent<T> {
    let component = Arc::new(Mutex::new(init));
    let state = component.clone();
    let (tx_out, rx_out) = txrx();
    let (tx_in, rx_in) = txrx();
    let subscriber = Subscriber::new(&tx_in);

    let (tx_view, rx_view) = txrx();
    rx_in.respond(move |msg:&T::ModelMsg| {
      let mut t =
        state
        .try_lock()
        .expect("Could not get component state lock");
      T::update(&mut t, msg, &tx_view, &subscriber);
    });

    let out_msgs = Arc::new(Mutex::new(vec![]));
    rx_view.respond(move |msg:&T::ViewMsg| {
      let should_schedule = {
        let mut msgs =
          out_msgs
          .try_lock()
          .expect("Could not try_lock to push to out_msgs");
        msgs.push(msg.clone());
        // If there is more than just this message in the queue, this
        // responder has already been run this frame and a timer has
        // already been scheduled, so there's no need to schedule another
        msgs.len() == 1
      };
      if should_schedule {
        let out_msgs_async = out_msgs.clone();
        let tx_out_async = tx_out.clone();
        utils::timeout(
          0,
          move || {
            let msgs = {
              out_msgs_async
                .try_lock()
                .expect("Could not try_lock to pop out_msgs")
                .drain(0..)
                .collect::<Vec<_>>()
            };
            if msgs.len() > 0 {
              msgs
                .iter()
                .for_each(|out_msg| {
                  tx_out_async.send(out_msg);
                });
            }
            false
          }
        );
      }
    });

    let builder =
      component
      .try_lock()
      .unwrap()
      .builder(tx_in.clone(), rx_out.branch());

    GizmoComponent {
      trns: tx_in,
      recv: rx_out,
      builder: Some(builder),
      gizmo: None,
      state: component
    }
  }

  /// Send model messages into this component from a `Receiver<T::ModelMsg>`.
  /// This is helpful for sending messages to this component from
  /// a parent component.
  pub fn rx_from(
    self,
    rx: Receiver<T::ModelMsg>
  ) -> GizmoComponent<T> {
    rx.forward_map(&self.trns, |msg| msg.clone());
    self
  }

  /// Send view messages from this component into a `Transmitter<T::ViewMsg>`.
  /// This is helpful for sending messages to this component from
  /// a parent component.
  pub fn tx_into(
    self,
    tx: &Transmitter<T::ViewMsg>
  ) -> GizmoComponent<T> {
    self
      .recv
      .branch()
      .forward_map(&tx, |msg| msg.clone());
    self
  }

  /// Build the GizmoComponent.builder. This will `take`
  /// the builder and update GizmoComponent.gizmo.
  pub fn build(&mut self) {
    if self.builder.is_some() {
      let builder =
        self
        .builder
        .take()
        .unwrap();
      self.gizmo =
        builder
        .build()
        .ok();
    }
  }

  /// Run and initialize the component with a list of messages.
  /// This is equivalent to calling `run` and `update` with each message.
  pub fn run_init(mut self, msgs: Vec<T::ModelMsg>) -> Result<(), JsValue> {
    msgs
      .into_iter()
      .for_each(|msg| {
        self.update(&msg);
      });
    self.run()
  }

  /// Run this component forever
  pub fn run(mut self) -> Result<(), JsValue> {
    if self.gizmo.is_none() && self.builder.is_some() {
      self.build();
    }

    self
      .gizmo
      .expect("Cannot run an unbuilt GizmoComponent")
      .run()
  }

  /// Append this component's gizmo an HtmlElement.
  /// Has no effect if this component has not been built.
  pub fn append_to(&self, parent: &HtmlElement) {
    if self.gizmo.is_some() {
      self
        .gizmo
        .as_ref()
        .unwrap()
        .append_to(parent);
    } else {
      warn!("Tried to append an un-built GizmoComponent to a parent - call 'build' first");
    }
  }

  /// Update the component with the given message.
  /// This how a parent component communicates down to its child components.
  pub fn update(&mut self, msg: &T::ModelMsg) {
    self.trns.send(msg);
  }

  /// Access the component's underlying state.
  pub fn with_state<F, N>(&self, f:F) -> N
  where
    F: Fn(&T) -> N
  {
    let t =
      self
      .state
      .try_lock()
      .expect("Could not get lock on GizmoComponent state");
    f(&t)
  }
}

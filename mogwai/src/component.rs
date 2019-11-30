//! Sometimes an application can get so entangled that it's hard to follow the
//! path of messages through `Transmitter`s, `Receiver`s and fold functions. For
//! situations like these where complexity is unavoidable, Mogwai provides &the
//! `Component` trait and the helper type `GizmoComponent`. Anyone familiar with
//! the Elm architecture will feel at home writing components in Mogwai:
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
//!   fn update(&mut self, msg: &In, _sub: &Subscriber<In>) -> Vec<Out> {
//!     match msg {
//!       In::Click => {
//!         self.num_clicks += 1;
//!         vec![Out::DrawClicks(self.num_clicks)]
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
//! In the example above we're creating a component that counts its own clicks.
//! The first step is to define the incoming messages that will update the model.
//! Next we define the outgoing messages that will update our view. The `builder`
//! trait method uses these message types to build the view. It does this by
//! consuming a `Transmitter<Self::ModelMsg>` and a `Receiver<Self::ViewMsg>`.
//! These represent the inputs and the outputs of your component. If your
//! component is owned by another, the parent component can communicate to the
//! child through these messages, either with `GizmoComponent::update` or by
//! subscribing to the messages when the child component is created.
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


/// Defines a component.
pub trait Component
where
  Self: Any + Sized,
  Self::ViewMsg: Clone
{
  /// A model message comes out from the view through a tx_on function into your
  /// component's update function.
  type ModelMsg;

  /// A view message comes out from your component's update function and changes
  /// the view by being used in an rx_* function.
  type ViewMsg;

  /// Update this component in response to any received model messages.
  /// This is essentially the component's fold function.
  /// Return any outgoing view messages.
  fn update(
    &mut self,
    msg: &Self::ModelMsg,
    sub: &Subscriber<Self::ModelMsg>
  ) -> Vec<Self::ViewMsg>;

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
    rx_in.respond(move |msg:&T::ModelMsg| {
      let out_msgs = {
        let mut t =
          state
          .try_lock()
          .expect("Could not get component state lock");
        T::update(&mut t, msg, &subscriber)
      };

      if out_msgs.len() > 0 {
        let tx_out_async = tx_out.clone();
        let out_msgs_async = out_msgs.clone();
        utils::timeout(0, move || {
          out_msgs_async
            .iter()
            .for_each(|out_msg| {
              tx_out_async.send(out_msg);
            });
          false
        });
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

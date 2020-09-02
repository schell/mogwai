//! Elmesque components through model and view message passing.
//!
//! Sometimes an application can get so entangled that it's hard to follow the
//! path of messages through `Transmitter`s, `Receiver`s and fold functions. For
//! situations like these where complexity is unavoidable, Mogwai provides the
//! [Component] trait and the helper struct [`Gizmo`].
//!
//! Many rust web app libraries use a message passing pattern made famous by
//! the Elm architecture to wrangle complexity. Mogwai is similar, but different
//! - Like other libraries, messages come out of the DOM into your component's
//!   model by way of the [Component::update] function.
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
//! [`Component::update`] method is given a [`Transmitter<Self::ViewMsg>`] with which
//! to send _view update messages_. Messages sent on this transmitter will
//! be sent out to the view to update the DOM. This forms a cycle. Messages
//! come into the update function from the view which processes, updates state,
//! and eventually sends messages out to the view, where they are used to update
//! the DOM.
//! In this way DOM updates are obvious. You know exactly where, when and
//! why updates are made - both to the model and the view.
//!
//! Here is a minimal example of a [`Component`] that counts its own clicks.
//!
//! ```rust, no_run
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! #[derive(Clone)]
//! enum In {
//!     Click
//! }
//!
//! #[derive(Clone)]
//! enum Out {
//!     DrawClicks(i32)
//! }
//!
//! struct App {
//!     num_clicks: i32
//! }
//!
//! impl Component for App {
//!     type ModelMsg = In;
//!     type ViewMsg = Out;
//!     type DomNode = HtmlElement;
//!
//!     fn view(&self, tx: &Transmitter<In>, rx: &Receiver<Out>) -> ViewBuilder<HtmlElement> {
//!         builder! {
//!             <button on:click=tx.contra_map(|_| In::Click)>
//!                 {(
//!                     "clicks = 0",
//!                     rx.branch_map(|msg| match msg {
//!                         Out::DrawClicks(n) => {
//!                             format!("clicks = {}", n)
//!                         }
//!                     })
//!                 )}
//!             </button>
//!         }
//!     }
//!
//!     fn update(&mut self, msg: &In, tx_view: &Transmitter<Out>, _sub: &Subscriber<In>) {
//!         match msg {
//!             In::Click => {
//!                 self.num_clicks += 1;
//!                 tx_view.send(&Out::DrawClicks(self.num_clicks));
//!             }
//!         }
//!     }
//! }
//!
//!
//! pub fn main() -> Result<(), JsValue> {
//!     Gizmo::from(
//!         App{ num_clicks: 0 }
//!     ).run()
//! }
//! ```
//!
//! As shown above, the first step is to define the incoming messages that will update the model.
//! Next we define the outgoing messages that will update our view. The [`Component::view`]
//! trait method uses these message types to build the view. It does this by
//! consuming a `Transmitter<Self::ModelMsg>` and a `Receiver<Self::ViewMsg>` and returning
//! a [`ViewBuilder`].
//! This channel represents the inputs and the outputs of your component. Roughly,
//! `Self::ModelMsg` comes into the [`Component::update`] function and `Self::ViewMsg`s go out
//! of the `update` function.
//!
//! ## Communicating to components
//!
//! If your component is owned by another, the parent component can communicate to
//! the child through its messages, either by calling [`Gizmo::update`]
//! on the child component within its own `update` function or by subscribing to
//! the child component's messages when the child component is created (see
//! [`Subscriber`]).
//!
//! ## Placing components
//!
//! Gizmos may be used within a [`View`] using the
//! [`ParentView::with`] function.
use wasm_bindgen::JsCast;
use web_sys::Node;

#[allow(unused_imports)]
use crate::prelude::{Gizmo, ParentView, Receiver, Transmitter, View, ViewBuilder};

pub mod subscriber;
use subscriber::Subscriber;

/// Defines a component with distinct input (model update) and output
/// (view update) messages.
///
/// See the [module level documentation](super::component) for more details.
pub trait Component
where
    Self: Sized + 'static,
    Self::ModelMsg: Clone,
    Self::ViewMsg: Clone,
    Self::DomNode: JsCast + AsRef<Node> + Clone,
{
    /// A model message comes out from the view through a tx_on function into your
    /// component's update function.
    type ModelMsg;

    /// A view message comes out from your component's update function and changes
    /// the view by being used in an rx_* function.
    type ViewMsg;

    /// The type of [`web_sys::Node`] that represents the root of this component.
    /// ie HtmlElement, HtmlInputElement, etc.
    type DomNode;

    /// Update this component in response to any received model messages.
    /// This is essentially the component's fold function.
    fn update(
        &mut self,
        msg: &Self::ModelMsg,
        tx_view: &Transmitter<Self::ViewMsg>,
        sub: &Subscriber<Self::ModelMsg>,
    );

    /// Produce this component's view using a `Transmitter` of model input messages
    /// and a `Receiver` of view output messages.
    ///
    /// Model messages flow from the view into the update function. View messages
    /// flow from the update function to the view.
    fn view(
        &self,
        tx: &Transmitter<Self::ModelMsg>,
        rx: &Receiver<Self::ViewMsg>,
    ) -> ViewBuilder<Self::DomNode>;
}

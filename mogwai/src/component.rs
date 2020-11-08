//! Reactive component trees using two way model and view message passing.
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
//! Instead of a virtual DOM Mogwai uses channels to patch the DOM from afar. The
//! [`Component::update`] method is given a [`Transmitter<Self::ViewMsg>`] with which
//! to send _view patching messages_. Messages sent on this transmitter will
//! be sent out to the view to update the DOM (if that view chooses to). This forms a
//! cycle:
//! 1. Messages come into the update function from the view which processes the message,
//! updates the state, and may send messages out to the view
//! 2. Message come into the view from the update function where they are used to patch
//! the DOM.
//!
//! In this way DOM updates are obvious. You know exactly where, when and
//! why updates are made - both to the model and the view.
//!
//! Here is a minimal example of a [`Component`] that counts its own clicks.
//!
//! ```rust
//! # extern crate mogwai;
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
//!     let app = Gizmo::from(App{ num_clicks: 0 });
//!
//!     if cfg!(target_arch = "wasm32") {
//!         View::from(app).run()
//!     } else {
//!         Ok(())
//!     }
//! }
//! ```
//!
//! As shown above, the first step is to define the incoming messages that will update the model.
//! Next we define the outgoing messages that will update our view. The [`Component::view`]
//! trait method uses these message types to build the view. It does this by
//! consuming a `Transmitter<Self::ModelMsg>` and a `Receiver<Self::ViewMsg>` and returning
//! a [`ViewBuilder`].
//! These terminals represent the inputs and the outputs of your component. Roughly,
//! `Self::ModelMsg` comes into the [`Component::update`] function and `Self::ViewMsg`s go out
//! of the `update` function.
//!
//! ## Creating a component
//!
//! To use a component after writing its `Component` trait implementation we turn it into a
//! [`Gizmo`]:
//!
//! ```rust, ignore
//!     let app: Gizmo<App> = Gizmo::from(App{ num_clicks: 0 });
//! ```
//!
//! [`Gizmo`]s can then be used to spawn a view, or can be converted into a view.
//!
//! ```rust, ignore
//!     let view = View::from(app.view_builder());
//! ```
//!
//! ```rust, ignore
//!     let view = View::from(app);
//! ```
//!
//! ## Communicating to components
//!
//! If your component is owned by another, the parent component can communicate to
//! the child through its messages, either by calling [`Gizmo::send`]
//! on the child component within its own update function or by subscribing to
//! the child component's messages when the child component is created (see
//! [`Subscriber`]).
//!
//! ## Placing components
//!
//! A parent component may nest an in-scope component by placing a [`ViewBuilder`]
//! or [`View`] inside the parent component's RSX:
//! ```rust, ignore
//! let child = builder! { <blockquote>"Fairies live"</blockquote> };
//! let parent = builder! {
//!     <div id="fairy_quote">{child}</div>
//! };
//! ```
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
    /// Message type used to drive component state updates.
    type ModelMsg;

    /// Message type used to drive view DOM patching.
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

    /// Used to perform any one-time binding from in scope [`Gizmo`]s to this component's subscriber.
    ///
    /// This should be used to bind sub-component view messages into this parent component's
    /// model messages in order to receive updates from child components. The default implementation
    /// is a noop.
    ///
    /// This function will be called only once, after a [`Gizmo`] is converted from the
    /// type implementing `Component`.
    #[allow(unused_variables)]
    fn bind(&self, sub: &Subscriber<Self::ModelMsg>) {}
}

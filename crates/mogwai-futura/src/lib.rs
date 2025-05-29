//! Future of mogwai.
//!
//! ## Impetus
//!
//! What I want is the ability to define a UI element and then render it
//! with various different platforms and have it behave similarly.
//!
//! An example of this would be creating a button that shows the number of
//! times it has been clicked, and then deploying that on the web, as a server-side
//! rendered string (after appying some number of artificial clicks) and also deploying
//! it in a terminal as a TUI.
//!
//! We might accomplish this with bare-bones Rust by defining the element in terms of a
//! model and a view interface. The model encodes the local state of the element
//! and its runtime logic, while the view interface determines how the runtime
//! logic can affect the view.
//!
//! The model is some concrete type, like `struct ButtonClicks {..}` and the view interface
//! would be a trait, `pub trait ButtonClicksInterface {..}`.
//!
//! Then each view platform ("web", "tui" and "ssr" in our case) could implement the view
//! interface and define the entry point.
//!
//! Model+logic and view.
//!
//! ### Model
//! Model is some concrete data that is used to update the view.
//! The type of the model cannot change from platform to platform.
//!
//! ### View Interface
//! A trait for interacting with the view in a cross-platform way.
//!
//! ### Logic
//! The logic is the computation that takes changes from the view through the interface,
//! updates the model and applies changes back through the interface.
//!
//! ### View
//! The view itself is responsible for rendering and providing events to the logic.
//! The type of the view changes depending on the platform.
//!
//! ## Strategy
//!
//! Mogwai's strategy towards solving the problem of cross-platform UI is not to offer
//! a one-size fits all view solution. Instead, `mogwai` aims to aid a _disciplined_
//! developer in modelling the UI using traits, and then providing the developer with
//! tools and wrappers to make fullfilling those traits on specific platforms as easy
//! as possible.
pub mod builder;
mod str;
pub mod sync;
pub mod time;
pub mod view;
#[cfg(feature = "web")]
pub mod web;

pub mod prelude {
    pub use crate::{builder::*, str::*, view::*};
}

pub use str::Str;

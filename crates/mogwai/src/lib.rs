//! # Mogwai: A Cross-Platform UI Library
//!
//! ## Overview
//!
//! Mogwai is designed to facilitate the creation of UI elements that can be rendered
//! across multiple platforms while maintaining consistent behavior. This framework
//! allows developers to define UI components in Rust, leveraging a model-view
//! architecture to separate concerns and enhance reusability.
//!
//! ### Key Concepts
//!
//! - **Model**: Represents the state and logic of a UI component. It is a concrete
//!   type that remains consistent across different platforms.
//!
//! - **View Interface**: A trait that defines how the model interacts with the view,
//!   enabling cross-platform compatibility.
//!
//! - **Logic**: The computation that processes input from the view, updates the model,
//!   and reflects changes back to the view.
//!
//! - **View**: Responsible for rendering the UI and handling events. The view's
//!   implementation varies depending on the target platform (e.g., web, TUI, SSR).
//!
//! ## Strategy
//!
//! Mogwai does not provide a one-size-fits-all solution for UI rendering. Instead, it
//! empowers developers to model their UI using traits, offering tools and wrappers to
//! implement these traits on specific platforms efficiently. This approach encourages
//! disciplined development and maximizes flexibility and control over the UI design.
pub mod proxy;
pub mod ssr;
mod str;
pub mod sync;
pub mod time;
pub mod view;
#[cfg(feature = "web")]
pub mod web;

pub mod prelude {
    pub use crate::{proxy::*, str::*, view::*};
}

pub use str::Str;

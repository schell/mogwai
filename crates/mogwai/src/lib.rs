//! # Mogwai: A Minimal Cross-Platform UI Library
//!
//! ## Overview
//!
//! Mogwai is a Rust-based framework designed to streamline the development of UI components
//! that can be rendered consistently across various platforms. By leveraging a model-view
//! architecture, Mogwai separates concerns, enhancing both reusability and maintainability
//! of UI components.
//!
//! ### Core Concepts
//!
//! - **Model**: Defines the state and logic of a UI component, ensuring consistency
//!   across different platforms.
//!
//! - **View Interface**: A trait that facilitates interaction between the model and the view,
//!   ensuring cross-platform compatibility.
//!
//! - **Logic**: Handles the processing of input from the view, updates the model, and
//!   reflects changes back to the view.
//!
//! - **View**: Manages the rendering of the UI and event handling, with implementations
//!   tailored to specific platforms (e.g., web, TUI, SSR).
//!
//! ## Approach
//!
//! Mogwai offers a flexible approach to UI rendering, allowing developers to model their
//! UI using traits. It provides tools and wrappers to efficiently implement these traits
//! on specific platforms, promoting disciplined development and maximizing control over
//! UI design.
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

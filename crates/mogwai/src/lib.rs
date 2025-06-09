//! # Mogwai: Cross-Platform UI Library
//!
//! Mogwai is a Rust framework for building UI components that work across platforms.
//! It uses a model-view architecture to separate concerns, enhancing reusability and maintainability.
//!
//! ## Key Concepts
//!
//! - **Model**: Manages state and logic.
//! - **View Interface**: Connects model and view for cross-platform compatibility.
//! - **Logic**: Processes input and updates the model.
//! - **View**: Renders UI and handles events, adaptable to platforms like web, TUI, and SSR.
//!
//! Mogwai provides tools to implement these concepts efficiently, promoting flexible and controlled UI design.
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

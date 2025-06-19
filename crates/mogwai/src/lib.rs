//! # Mogwai: Cross-Platform UI Library
//!
//! Mogwai is a Rust library for building UI components that work across platforms,
//! but primarily in the browser.
//!
//! ## Key Concepts
//!
//! - **Low boilerplate view construction**: Use the [`rsx!`](view::rsx) macro to reduce boilerplate.
//! - **Async event handling**: Events are futures, not callbacks.
//! - **Cross-platform support**: [View traits](crate::view) ensure operations are cross-platform,
//!   with room for specialization.
//! - **Idiomatic Rust**: Widgets are Rust types
//!
//! Mogwai provides tools to implement these concepts efficiently, promoting flexibility and performance.
pub mod an_introduction;
#[cfg(feature = "future")]
pub mod future;
pub mod proxy;
#[cfg(feature = "ssr")]
pub mod ssr;
mod str;
pub mod sync;
pub mod time;
pub mod view;
#[cfg(feature = "web")]
pub mod web;

pub mod prelude {
    //! Common prelude between all platforms.
    pub use crate::{proxy::*, str::*, view::*};
}

pub use str::Str;

#[cfg(doctest)]
doc_comment::doctest!("../../../README.md", readme);

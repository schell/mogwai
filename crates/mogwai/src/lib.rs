#![warn(missing_docs)]
#![allow(deprecated)]
//! # Mogwai
//!
//! Mogwai is library for user interface development using Rust-to-Wasm
//! compilation. Its goals are simple:
//! * provide a declarative approach to creating and managing DOM nodes
//! * encapsulate component state and compose components easily
//! * explicate DOM updates
//! * feel snappy
//!
//! ## Learn more
//! If you're new to Mogwai, check out the [introduction](an_introduction) module.
pub mod an_introduction;
pub mod builder;
pub mod channel;
pub mod component;
pub mod event;
pub mod futures;
pub mod model;
pub mod patch;
pub mod prelude;
pub mod relay;
pub mod ssr;
pub mod target;
pub mod time;
pub mod utils;
pub mod view;

pub use target::spawn;

pub mod lock {
    //! Asynchronous locking mechanisms (re-exports).
    pub use async_lock::*;
    pub use futures::lock::*;
}

pub mod macros {
    //! RSX style macros for building DOM views.
    pub use mogwai_html_macro::{builder, view};
}

#[cfg(doctest)]
doc_comment::doctest!("../../../README.md");

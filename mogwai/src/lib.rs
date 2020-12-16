#![warn(missing_docs)]
//! # Mogwai
//!
//! Mogwai is library for frontend web development using Rust-to-Wasm
//! compilation. Its goals are simple:
//! * provide a declarative approach to creating and managing DOM nodes
//! * encapsulate component state and compose components easily
//! * explicate DOM updates
//! * feel snappy
//!
//! ## Learn more
//! If you're new to Mogwai, check out the [introduction](an_introduction) module.
pub mod an_introduction;
pub mod component;
pub mod gizmo;
pub mod model;
pub mod prelude;
#[cfg(not(target_arch = "wasm32"))]
pub mod ssr;
pub mod txrx;
pub mod utils;
pub mod view;

#[cfg(doctest)]
doc_comment::doctest!("../../README.md");

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
//! If you're new to Mogwai, check out the [introduction](introduction) module.
extern crate console_log;
extern crate either;
#[macro_use]
extern crate log;
extern crate web_sys;

pub mod builder;
pub mod component;
pub mod txrx;
pub mod gizmo;
pub mod prelude;
pub mod utils;
pub mod introduction;

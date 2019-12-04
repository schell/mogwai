//! # Mogwai
//!
//! Mogwai is library for frontend web development using Rust-to-Wasm
//! compilation. Its goals are simple:
//! * provide a declarative approach to creating and managing DOM nodes
//! * encapsulate component state and compose components easily
//! * explicate DOM updates
//! * feel snappy
//!
//! ## ethos
//! Mogwai is cognitively small and runtime fast. It acheives these goals
//! by doing only few things, but doing those things well.
//!
//! ### Building DOM
//! Building DOM is one of the main authorship modes in Mogwai. DOM nodes
//! are created using a builder pattern. The builder itself is called
//! `GizmoBuilder`. It looks like this:
//!
//! ```rust, no_run
//! extern crate mogwai;
//! use::mogwai::prelude::*;
//!
//! div()
//!   .class("my-div")
//!   .with(
//!     a()
//!       .attribute("href", "https://zyghost.com")
//!       .text("Schell's website")
//!   )
//!   .build().unwrap()
//!   .run().unwrap()
//! ```
//!
//! The example above would create a DOM node and appends it to the document
//! body. It would look like this:
//!
//! ```html
//! <div class="my-div">
//!   <a href="https://zyghost.com">Schell's website</a>
//! </div>
//! ```
//!
//! ### Wiring DOM
//! `GizmoBuilder`s can be static like the one above, or they can be dynamic.
//! Dynamic `GizmoBuilder`s get their values from the receiving end of a channel.
//! Whenever the sending end of the channel sends a value, the DOM is updated.
//!
//! ```rust, no_run
//! extern crate mogwai;
//! use::mogwai::prelude::*;
//!
//! let (tx, rx) = txrx();
//!
//! div()
//!   .class("my-div")
//!   .with(
//!     a()
//!       .attribute("href", "https://zyghost.com")
//!       .rx_text("Schell's website", rx)
//!   )
//!   .build().unwrap()
//!   .run().unwrap()
//!
//! tx.send("My website");
//! ```
//!
//! Just like previously, this builds a DOM node and appends it to the document
//! body, but this time we've already updated the link's text to "My website":
//!
//! ```html
//! <div class="my-div">
//!   <a href="https://zyghost.com">My website</a>
//! </div>
//! ```
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

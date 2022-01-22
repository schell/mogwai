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
pub mod prelude;

pub mod core {
    //! Re-export of `mogwai-core`. Core types and traits.
    pub use mogwai_core::*;
}

#[cfg(feature = "dom")]
pub mod dom {
    //! Re-export of `mogwai-dom` using the "dom" feature. Browser + server html
    //! views.
    pub use mogwai_dom::*;
}

pub use mogwai_core::target::spawn;

pub mod macros {
    //! Rexexport of `mogwai-macros`. RSX style macros for building views.
    pub use mogwai_macros::{builder, rsx, view};
}

#[cfg(doctest)]
doc_comment::doctest!("../../../README.md");

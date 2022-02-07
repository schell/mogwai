#![warn(missing_docs)]
#![allow(deprecated)]
//! # Mogwai
//!
//! Mogwai is library for multi-domain user interface development using sinks and streams.
//!
//! Its goals are simple:
//! * provide a declarative approach to creating and managing interface nodes, without
//!   a bias towards a specific UI domain (ie web, games, desktop applications, mobile)
//! * encapsulate component state and compose components easily
//! * explicate mutations and updates
//! * feel snappy
//!
//! ## Learn more
//! If you're new to Mogwai, check out the [introduction](an_introduction) module.
//!
//! ## Acronyms
//! If you're wondering what the acronym "mogwai" stands for, here is a table of
//! options that work well, depending on the domain. It's fun to mix and match.
//!
//! | M           | O         | G           | W      | A             | I            |
//! |-------------|-----------|-------------|--------|---------------|--------------|
//! | minimal     | obvious   | graphical   | web    | application   | interface    |
//! | modular     | operable  | graphable   | widget |               |              |
//! | mostly      |           | gui         | work   |               |              |
pub mod an_introduction;

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
    pub use mogwai_macros::{builder, html, rsx, view};
}

pub mod prelude;

#[cfg(doctest)]
doc_comment::doctest!("../../../README.md");

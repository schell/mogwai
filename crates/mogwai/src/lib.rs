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

pub use mogwai_core::*;
pub use mogwai_macros::*;

#[cfg(any(feature = "dom", feature = "dom-wasm"))]
pub mod dom {
    //! Re-exports of [`mogwai_dom`].
    pub use mogwai_dom::*;
}

/// Spawn an async computation.
///
/// The implementation of `spawn` depends on the features used to compile.
/// With `dom` or `dom-wasm` the implementation will be `mogwai_dom::spawn`.
///
/// ## Panics
/// Not all view domains have `spawn`, or they may provide a different API.
/// In those cases this function will panic.
pub fn spawn<T: Send + Sync + 'static>(_f: impl constraints::Spawnable<T>) {
    #[cfg(any(feature = "dom", feature = "dom-wasm"))]
    {
        dom::spawn(_f);
    }

    #[cfg(all(not(feature = "dom"), not(feature = "dom-wasm")))]
    {
        panic!("spawn has no implementation with these features");
    }
}

pub mod prelude;

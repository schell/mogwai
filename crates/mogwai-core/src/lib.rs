//! # Mogwai
//!
//! Mogwai is library for user interface development using sinks and streams.
//!
//! Its goals are simple:
//! * provide a declarative way to create and manage interface nodes, without
//!   a bias towards a specific UI domain (ie web, games, desktop applications, mobile)
//! * encapsulate component state and compose components easily
//! * explicate mutations and updates
//! * feel snappy
//! * allow the library user to access the underlying raw views when necessary, ie - you have
//!   an "escape hatch"
//!
//! ## UI Domains
//! Mogwai has domain-specific libraries for certain user interface domains that re-export this
//! core library and specialize it for the domain.
//!
//! ### Javascript/Browser DOM
//! TODO: Write about `mogwai-dom`
//!
//! ### Terminal UI
//! TODO: implement `mogwai-tui`
//!
//! ### Write your own
//! TODO: Explain how to write your own domain-specific mogwai library
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
//!
pub mod channel;
pub mod futures;
pub mod model;
pub mod patch;
pub mod relay;
pub mod time;
pub mod view;
pub use mogwai_macros::{rsx, html, builder};

pub mod prelude {
    //! Re-exports for convenience
    pub use super::view::*;
    pub use super::{rsx, html, builder};
    pub use super::futures::{Stream, StreamExt, Sink, SinkExt, sink::Contravariant, Captured};
    pub use super::patch::{HashPatch, HashPatchApply, ListPatch, ListPatchApply};
    pub use super::relay::*;
    pub use super::channel::SinkError;
}

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
//! [mogwai-dom](https://crates.io/crates/mogwai-dom) is a library for building browser applications
//! using the Javascript API. It is a mogwai wrapper around [web_sys](https://crates.io/crates/web-sys).
//!
//! ### Terminal UI
//! TODO: implement `mogwai-tui`
//!
//! ### Write your own
//! Would you like to be able to build mogwai apps in a domain that doesn't exist yet?
//! You can build it! For the most part the bulk of the work is writing an implementation of
//! `TryFrom<ViewBuilder>` (or a similar conversion) for your domain-specific view type. See the
//! [`mogwai-dom` source code](https://github.com/schell/mogwai/tree/no-constraints/crates/mogwai-dom)
//! for an example of prior art.
//!
//! TODO: change the link for `mogwai-dom` to `main` after merging
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
pub mod either;
pub mod future;
pub mod sink;
pub mod stream;
pub mod model;
pub mod patch;
pub mod relay;
pub mod time;
pub mod view;
pub use mogwai_macros::{builder, html, rsx};

pub mod prelude {
    //! Re-exports for convenience
    pub use super::future::Captured;
    pub use super::sink::{SendError, Sink, SinkExt, TrySendError};
    pub use super::stream::{Stream, StreamExt};
    pub use super::patch::{HashPatch, HashPatchApply, ListPatch, ListPatchApply};
    pub use super::relay::*;
    pub use super::view::*;
    pub use super::{builder, html, rsx};
}

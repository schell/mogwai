#![allow(unused_braces)]
//! An introduction to the minimal, obvious, graphical web application interface.
//!
//! # Welcome!
//! This is a library for building asynchronous user interfaces.
//! The following is a short introduction to the library's basic concepts.
//!
//! ## Channels, Sinks and Streams
//! Sinks and streams are used for asynchronous communication between view
//! and logic.
//!
//! ### Sinks
//! A [`Sink`] is something you can send values **into**, like the sending
//! end of a channel. See [`mogwai::futures::SinkExt`] for info on the other
//! sink operations available.
//!
//! ### Streams
//! A [`Stream`] is something you can get values **out of**, like the receiving
//! end of a channel. True to its name, a `Stream` is a stream of values in time
//! that may end at some point in the future. See [`mogwai::futures::StreamExt`]
//! for info on the other stream operations available.
//!
//! ### Channels
//! Being trait objects, sinks and streams are a bit abstract. In this library
//! the concrete types that provide the implementation of sink and stream are
//! the ends of a channel.
//!
//! There are two types of channels bundled here:
//!
//! - [`mogwai::channel::broadcast`]
//!   This should be the channel you use most often. If you don't have a specific
//!   reason not to, use this channel.
//!
//! - [`mogwai::channel::mpsc`]
//!   This is used to send patches of [`ViewBuilder`] (more on that later) and any
//!   other type that does not have a `Clone` implementation.
//!
//! Both channels' `Sender` are `Sink` and both channels' `Receiver` are `Stream`.
//!
//! You are not limited to using this library's provided channels. Any sink or stream
//! should work just fine.
//!
//! ## Constructing views
//!
//! This library can be used to construct many types of domain-specific views,
//! but for the remainder of the introduction we will be talking about web browser-based
//! DOM views.
//!
//! Building DOM is one of the primary tasks of web development. With mogwai the
//! quickest way to construct DOM nodes is by using the RSX macro [`builder`].
//!
//! [`builder!`] and [`view!`] are flavors of mogwai's RSX that evaluate to
//! [`ViewBuilder`] or [`View`] respectively. RSX is a lot like react.js's JSX,
//! except that it uses type checked rust expressions.
//!
//! Most of the time you'll see the [`builder!`] macro used to create a [`ViewBuilder`]:
//!
//! ```rust
//! use::mogwai::prelude::*;
//!
//! let my_div: ViewBuilder<Dom> = builder!{
//!     <div class="my-div">
//!         <a href="http://zyghost.com">
//!             "Schellsan's website"
//!         </a>
//!     </div>
//!   };
//! ```
//!
//! [`ViewBuilder`] can be converted into a domain specific view.
//! Here we're creating `Dom` from `mogwai-dom` for use in the browser:
//!
//! ```rust
//! use::mogwai::prelude::*;
//! use std::convert::TryFrom;
//!
//! let my_div: ViewBuilder<Dom> = builder!(
//!     <div class="my-div">
//!         <a href="http://zyghost.com">
//!             "Schellsan's website"
//!         </a>
//!     </div>
//!   );
//! let view: Dom = my_div.build().unwrap();
//!
//! let html: String = smol::block_on(async { view.html_string().await });
//! assert_eq!(
//!     html,
//!     r#"<div class="my-div"><a href="http://zyghost.com">Schellsan's website</a></div>"#
//! );
//! ```
//!
//! > #### Note
//! > The [`view`] macro creates a builder, builds it and unwraps it all in one go.
//!
//! As you can see the above example creates a DOM node with a link inside it:
//!
//! ```html
//! <div class="my-div">
//!       <a href="http://zyghost.com">Schellsan's website</a>
//! </div>
//! ```
//!
//! A view is a domain-specific view type. In this case that's
//! [`mogwai_dom::view::Dom`]. It's responsible for view mutation.
//!
//! ### Appending a built view to the DOM
//!
//! To append a `Dom` to the `document.body` we can use [`Dom::run`]:
//!
//! ```rust, no_run
//! use::mogwai::prelude::*;
//!
//! let my_div: ViewBuilder<Dom> = builder!(
//!     <div class="my-div">
//!         <a href="http://zyghost.com">
//!             "Schellsan's website"
//!         </a>
//!     </div>
//!   );
//! let dom: Dom = my_div.build().unwrap();
//! dom.run().unwrap();
//! ```
//!
//! [`Dom::run`] consumes the view, *handing ownership to the browser window*.
//!
//! ### Detaching [`Dom`]
//!
//! `Dom` can be detached from its parent using [`Dom::detach`].
//!
//! ### Dynamic views
//!
//! A view may be static like the one above but more often they change over time.
//! Views get their dynamic values from streams:
//!
//! ```rust
//! use::mogwai::prelude::*;
//!
//! smol::block_on(async {
//!     let (mut tx, rx) = broadcast::bounded(1);
//!
//!     let my_view = view!{
//!         <div class="my-div">
//!             <a href="http://zyghost.com">
//!                 // start with a value and update when a message
//!                 // is received on rx.
//!                 {("Schellsan's website", rx)}
//!             </a>
//!         </div>
//!     };
//!
//!     tx.send("Gizmo's website".to_string()).await.unwrap();
//! });
//! ```
//!
//! A [`broadcast::Sender`] can be used to send DOM events as messages, allowing
//! your view to communicate with itself or other components:
//! ```rust
//! use::mogwai::prelude::*;
//!
//! let (tx, rx) = broadcast::bounded(1);
//!
//! let my_view = view!{
//!     <div class="my-div">
//!         <a href="#" on:click=tx.contra_map(|_: DomEvent| "Gizmo's website".to_string())>
//!             // start with a value and update when a message
//!             // is received on rx.
//!             {("Schellsan's website", rx)}
//!         </a>
//!     </div>
//! };
//! ```
//!
//! The [`Contravariant`] trait provides a few useful functions for prefix-mapping sinks, which is used
//! above. See [futures's module level documentation](super::futures) for more info on mapping, folding and
//! combining `Sink`s and `Stream`s.
//!
//! ### Accessing the underlying raw node
//!
//! [`Dom`] contains a reference to the raw DOM node, making it possible
//! to manipulate the DOM by hand using Javascript FFI bindings and functions
//! provided by the great `web_sys` crate:
//!
//! ```rust, no_run
//! use::mogwai::prelude::*;
//!
//! mogwai::spawn(async {
//!     let (mut tx, rx) = broadcast::bounded(1);
//!
//!     let my_view = view!{
//!         <div class="my-div">
//!             <a href="http://zyghost.com">
//!                 // start with a value and update when a message
//!                 // is received on rx.
//!                 {("Schellsan's website", rx)}
//!             </a>
//!         </div>
//!     };
//!     tx.send("Gizmo's website".into()).await.unwrap();
//!
//!     // only `Some` in the browser when compiled for wasm32
//!     if let Some(el) = my_view.clone_as::<web_sys::HtmlElement>() {
//!         assert_eq!(
//!             el.inner_html(),
//!             r#"<a href="http://zyghost.com">Gizmo's website</a>"#
//!         );
//!     }
//! });
//! ```
//!
//! ### Components and more advanced widgets
//!
//! For anything but the simplest view, it is recommended you use the
//! [`Component`] and [`ElmComponent`] structs to build your view components.
//!
//! In bigger applications we often have circular dependencies between buttons,
//! fields and other interface elements. When these complex situations arise
//! we compartmentalize concerns into [`Component`]s.
//!
//! Other times we don't need a full component with its own logic and instead
//! we simply require a [`ViewBuilder`].
//!
//! ## JavaScript interoperability
//! The library itself is a thin layer on top of the
//! [web-sys](https://crates.io/crates/web-sys) crate which provides raw bindings
//! to _tons_ of browser web APIs. Many of the DOM specific structs, enums and
//! traits come from `web-sys`. Some of those types are re-exported by Mogwai's
//! [prelude](../prelude). The most important trait to understand for the
//! purposes of this introduction (and for writing web apps in Rust, in general)
//! is the [`JsCast`](../prelude/trait.JsCast.html) trait. Its `dyn_into` and
//! `dyn_ref` functions are the primary way to cast JavaScript values as specific
//! types.
#[allow(unused_imports)]
use super::prelude::*;
#[allow(unused_imports)]
use crate as mogwai;

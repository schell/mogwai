#![allow(unused_braces)]
//! An introduction to writing browser interfaces with mogwai.
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
//! Mogwai can be used to construct many types of domain-specific views,
//! but for the remainder of the introduction we will be talking about web browser-based
//! DOM views.
//!
//! Building DOM is one of the primary tasks of web development. With mogwai the
//! quickest way to construct DOM nodes is by using the [`rsx`] or [`html`] macros.
//!
//! These macros are flavors of mogwai's RSX that evaluate to
//! [`ViewBuilder`]. RSX is a lot like react.js's JSX, except that it uses type checked
//! rust expressions.
//!
//! Most of the time you'll see the [`rsx!`] macro used to create a [`ViewBuilder`]:
//!
//! ```rust
//! use mogwai_dom::prelude::*;
//!
//! let my_div: ViewBuilder = rsx!{
//!     div(class="my-div") {
//!         a(href="http://zyghost.com") {
//!             "Schellsan's website"
//!         }
//!     }
//!   };
//! ```
//!
//! [`ViewBuilder`] can be converted into a domain specific view.
//! Here we're creating `mogwai_dom::view::Dom` for use in the either the browser
//! or server-side rendering:
//!
//! ```rust
//! use::mogwai_dom::prelude::*;
//! use std::convert::TryFrom;
//!
//! let my_div: ViewBuilder = rsx!{
//!     div(class="my-div") {
//!         a(href="http://zyghost.com") {
//!             "Schellsan's website"
//!         }
//!     }
//!   };
//! let view = Dom::try_from(my_div).unwrap();
//!
//! let html: String = futures::executor::block_on(async { view.html_string().await });
//! assert_eq!(
//!     html,
//!     r#"<div class="my-div"><a href="http://zyghost.com">Schellsan's website</a></div>"#
//! );
//! ```
//!
//! As you can see the above example creates a browser DOM node with a link inside it:
//!
//! ```html
//! <div class="my-div">
//!       <a href="http://zyghost.com">Schellsan's website</a>
//! </div>
//! ```
//!
//! [`Dom`] is responsible for taking the `ViewBuilder`'s various streams of updates and
//! mutating in response, but those are implementation details we don't need to talk about here.
//!
//! In `mogwai-dom` there are three view types.
//! * [`JsDom`] represents a Javascript-owned browser DOM element. This is the type to
//!   use when building apps to run in the browser. It can only be run when built for
//!   WASM.
//! * [`SsrDom`] represents a server-side-rendered DOM element.
//! * [`Dom`] represents either [`JsDom`] or [`SsrDom`] depending on what architecture the
//!   Rust program has been built for.
//!
//! ### Appending a built view to the DOM
//!
//! To append a `JsDom` to the `document.body` we can use [`JsDom::run`]:
//!
//! ```rust, no_run
//! use::mogwai_dom::prelude::*;
//!
//! let my_div: ViewBuilder = rsx!(
//!     div(class="my-div") {
//!         a(href="http://zyghost.com") {
//!             "Schellsan's website"
//!         }
//!     }
//! );
//! let dom  = JsDom::try_from(my_div).unwrap();
//! dom.run().unwrap();
//! ```
//!
//! The `run` function consumes the view, attaching it to
//! the `document.body` and *never dropping the node*.
//!
//! ### Detaching [`Dom`]
//!
//! `Dom` can be detached from its parent using [`Dom::detach`]. This happens automatically
//! when patching a node's children with streams. We'll talk more about that later.
//!
//! ### Dynamic views
//!
//! A view may be static like the one above but more often they change over time.
//! Views get their dynamic values from streams:
//!
//! ```rust
//! use mogwai_dom::prelude::*;
//! use mogwai_dom::core::channel::broadcast;
//!
//! futures::executor::block_on(async {
//!     let (mut tx, rx) = broadcast::bounded(1);
//!
//!     let my_view = SsrDom::try_from(rsx!{
//!         div(class="my-div") {
//!             a(href="http://zyghost.com") {
//!                 // start with a value and update when a message
//!                 // is received on rx.
//!                 {("Schellsan's website", rx)}
//!             }
//!         }
//!     }).unwrap();
//!
//!     tx.send("Gizmo's website".to_string()).await.unwrap();
//! });
//! ```
//!
//! A [`broadcast::Sender`] can be used to send DOM events as messages, allowing
//! your view to communicate with itself or other components:
//! ```rust
//! use::mogwai_dom::prelude::*;
//! use::mogwai_dom::core::channel::broadcast;
//!
//! let (tx, rx) = broadcast::bounded(1);
//!
//! let my_view = Dom::try_from(rsx!{
//!     div(class="my-div") {
//!         a(href="#", on:click=tx.contra_map(|_: DomEvent| "Gizmo's website".to_string())) {
//!             // start with a value and update when a message
//!             // is received on rx.
//!             {("Schellsan's website", rx)}
//!         }
//!     }
//! }).unwrap();
//! ```
//!
//! The [`Contravariant`] trait provides a few useful functions for prefix-mapping sinks, which is used
//! above. See [futures's module level documentation](mogwai::futures) for more info on mapping, folding and
//! combining `Sink`s and `Stream`s.
//!
//! ### Accessing views
//! [`Dom`] contains a reference to the raw Javascript DOM node when built on WASM,
//! making it possible to manipulate the DOM by hand using Javascript FFI bindings and functions
//! provided by the great `web_sys` crate:
//!
//! ```rust
//! use mogwai_dom::prelude::*;
//! use mogwai_dom::core::channel::broadcast;
//!
//! futures::executor::block_on(async {
//!     let (mut tx, rx) = broadcast::bounded(1);
//!
//!     let my_view = Dom::try_from(rsx!{
//!         div(class="my-div") {
//!             a(href="http://zyghost.com") {
//!                 // start with a value and update when a message
//!                 // is received on rx.
//!                 {("Schellsan's website", rx)}
//!             }
//!         }
//!     })
//!     .unwrap();
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
//! ### More advanced widgets
//!
//! #### Logic
//!
//! A [`ViewBuilder`] may contain asynchronous logic. Use [`ViewBuilder::with_task`] to
//! add an asynchronous task that will be spawned at view build time.
//!
//! #### Nesting
//!
//! [`ViewBuilder`]s may be nested to build up trees of widgets.
//! Please see the module level documentation for more info.
//!
//! #### Relays
//!
//! In bigger applications we often have circular dependencies between various
//! interface components. When these complex situations arise we can compartmentalize concerns into
//! relays.
//!
//! View relays are custom structs made in part by types in the [`relay`] module that contain the inputs
//! and outputs of your view. They should be converted into [`ViewBuilder`]s and can be used to
//! communicate and control your views. If used correctly a relay can greatly reduce the complexity
//! of your application.
//! Please see the module level documentation for more info.

#[allow(unused_imports)]
use super::prelude::*;
#[allow(unused_imports)]
use crate as mogwai_dom;

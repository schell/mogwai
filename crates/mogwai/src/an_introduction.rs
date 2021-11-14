#![allow(unused_braces)]
//! An introduction to the minimal, obvious, graphical web application interface.
//!
//! # Welcome!
//! Mogwai is a cute little library for building interfaces. It is
//! cognitively small and runtime fast. It acheives these goals by doing very few
//! things, but doing those things well.
//!
//! The following is a short introduction to the concepts of Mogwai.
//!
//! ## Channels
//!
//! Async channels are used for communication between views and logic. There are
//! two types of channels bundled by Mogwai:
//!
//! - [`mogwai::channel::mpmc::bounded`], exported from `async-channel`
//! - [`mogwai::channel::broadcast::bounded`], exported from `async-broadcast`
//!
//! ## Constructing DOM Nodes
//!
//! Building DOM is one of the main tasks of web development. In mogwai the
//! quickest way to construct DOM nodes is by using built in RSX macros.
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
//! let my_div: ViewBuilder<Dom> = builder!(
//!     <div class="my-div">
//!         <a href="http://zyghost.com">
//!             "Schellsan's website"
//!         </a>
//!     </div>
//!   );
//! ```
//!
//! [`ViewBuilder`] can be converted into [`View`]:
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
//! let view: View<Dom> = View::try_from(my_div).unwrap();
//!
//! assert_eq!(
//!     String::from(view),
//!     r#"<div class="my-div"><a href="http://zyghost.com">Schellsan's website</a></div>"#
//! );
//! ```
//!
//! As you can see the above example creates a DOM node with a link inside it:
//!
//! ```html
//! <div class="my-div">
//!       <a href="http://zyghost.com">Schell's website</a>
//! </div>
//! ```
//!
//! ### Appending a [`View`] to the DOM
//!
//! To append a [`View`] to the DOM's `document.body` we can use [`View::run`]:
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
//! let view: View<Dom> = View::try_from(my_div).unwrap();
//! view.run().unwrap();
//! ```
//!
//! [`View::run`] consumes the view, *handing ownership to the browser window*.
//!
//! ### Dropping a [`View`]
//!
//! By handing the [`View`] off to the window it never goes out of scope.
//! This is important - when a [`View`] is dropped, the inner DOM node is
//! removed from the DOM.
//!
//! ### Wiring DOM
//!
//! [`View`]s can be static like the one above, or they can change over time.
//! [`View`]s get their dynamic values from the receiving end of a channel
//! called a `Receiver<T>`. The sending end of the channel is called a
//! `Sender<T>`. This should be somewhat familiar if you've ever used a
//! channel in other rust libraries.
//!
//! ```rust
//! use::mogwai::prelude::*;
//!
//! mogwai::spawn(async {
//!     let (tx, rx) = broadcast::bounded(1);
//!
//!     let my_view = view!(
//!         <div class="my-div">
//!             <a href="http://zyghost.com">
//!                 // start with a value and update when a message
//!                 // is received on rx.
//!                 {("Schellsan's website", rx)}
//!             </a>
//!         </div>
//!     );
//!
//!     tx.broadcast(&"Gizmo's website".into()).await.unwrap();
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
//!         <a href="#" on:click=tx.sink().contra_map(|_: Event| "Gizmo's website".to_string())>
//!             // start with a value and update when a message
//!             // is received on rx.
//!             {("Schellsan's website", rx)}
//!         </a>
//!     </div>
//! };
//! ```
//!
//! A [`broadcast::Sender`] can be converted into a [`Sink`], while a [`broadcast::Receiver`]
//! implements [`Stream`] directly.
//! See [channel's module level documentation](super::channel) for more info on mapping, folding and
//! combining `Senders`s and `Receiver`s.
//!
//! ### Accessing the underlying DOM node
//!
//! The [`View`] contains a reference to the raw DOM node, making it possible
//! to manipulate the DOM by hand using Javascript FFI bindings and functions
//! provided by the great `web_sys` crate:
//!
//! ```rust, no_run
//! use::mogwai::prelude::*;
//!
//! mogwai::spawn(async {
//!     let (tx, rx) = broadcast::bounded(1);
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
//!     tx.broadcast(&"Gizmo's website".into()).await.unwrap();
//!
//!     let html = my_view.visit_as::<web_sys::HtmlElement>(
//!         |el:&HtmlElement| el.inner_html(), // closure ran on browser when compiled for wasm32
//!         |el:&SsrElement<Event>| String::from(el), // closure ran server-side when compiled on other targets
//!     );
//!     assert_eq!(
//!         html,
//!         r#"<a href="http://zyghost.com">Gizmo's website</a>"#
//!     );
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

#![allow(unused_braces)]
//! An introduction to writing browser interfaces with mogwai.
//!
//! # Welcome!
//! This is a library for building asynchronous user interfaces.
//! The following is a short introduction to the library's basic concepts.
//!
//! ## Asynchronous Communication
//!
//! Mogwai uses asynchronous communication to manage interactions between views
//! and logic. This is achieved through the use of futures and proxies, which
//! allow for dynamic updates and event handling.
//!
//! ### Futures
//! Futures represent values that will be available at some point in the future.
//! They are used extensively in Mogwai to handle asynchronous events and updates.
//!
//! ### Proxies
//! Proxies are used to manage state that can change over time. They provide a
//! mechanism for updating views dynamically when the underlying data changes.
//!
//! ## View Construction
//!
//! Mogwai can be used to construct many types of domain-specific views,
//! but for the remainder of the introduction we will be talking about web
//! browser-based DOM views.
//!
//! Building DOM is one of the primary tasks of web development. With mogwai the
//! quickest way to construct DOM nodes is by using the [`rsx`] macro.
//!
//! This macro is a flavor of mogwai's RSX that evaluates to
//! [`ViewBuilder`]. RSX is a lot like react.js's JSX, except that it uses type
//! checked rust expressions.
//!
//! ## Constructing views
//!
//! Mogwai can be used to construct many types of domain-specific views,
//! but for the remainder of the introduction we will be talking about web
//! browser-based DOM views.
//!
//! Most of the time you'll see the [`rsx!`] macro used to create a view:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! let my_div = rsx! {
//!     div(class = "my-div") {
//!         a(href = "http://zyghost.com") {
//!             "Schellsan's website"
//!         }
//!     }
//! };
//! ```
//!
//! The `rsx!` macro can be used to create views that are compatible with different
//! platforms. Here we're creating a view for use in the browser:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! let my_div = rsx! {
//!     div(class = "my-div") {
//!         a(href = "http://zyghost.com") {
//!             "Schellsan's website"
//!         }
//!     }
//! };
//!
//! // Convert the view to a specific platform, such as Web or SSR
//! let view: Web::Element = my_div.into();
//! ```
//!
//! The above example creates a browser DOM node with a link inside it. The `rsx!`
//! macro allows for the creation of complex views with minimal boilerplate.
//!
//! ## Dynamic Views
//!
//! Views in Mogwai can be dynamic, updating in response to changes in state or
//! events. This is achieved through the use of proxies and futures, which allow
//! for asynchronous updates and event handling.
//!
//! ## Dynamic Views
//!
//! Views in Mogwai can be dynamic, updating in response to changes in state or
//! events. This is achieved through the use of proxies and futures, which allow
//! for asynchronous updates and event handling.
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! let mut state = Proxy::new("Initial state".to_string());
//!
//! let view = rsx! {
//!     div() {
//!         p() { {state.clone()} }
//!         button(on:click = |_| state.set("Updated state".to_string())) {
//!             "Update State"
//!         }
//!     }
//! };
//! ```
//!
//! In this example, clicking the button updates the state, which in turn updates
//! the paragraph text. The `Proxy` type is used to manage the state and trigger
//! updates to the view when the state changes.

#[allow(unused_imports)]
use super::prelude::*;
#[allow(unused_imports)]
use crate as mogwai_dom;

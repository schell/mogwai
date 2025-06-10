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
//! They are used extensively in Mogwai to handle events.
//!
//! ## Mutation and data updates
//!
//! The [`Proxy`] type is used to update multiple places in a view from a single
//! data update. We'll talk more about this later.
//!
//! ## View Construction
//!
//! View construction is accomplished using a novel [`rsx!`] macro that reduces
//! boilerplate and has special syntax for use with [`Proxy`].
//!
//! ### RSX
//!
//! [`rsx!`] is a lot like react.js's JSX, except that it uses type checked rust expressions.
//!
//! Let's start by writing a function that constructs a simple element:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct Widget<V:View> {
//!     root: V::Element
//! }
//!
//! impl<V:View> Widget<V> {
//!     fn new() -> Self {
//!         rsx! {
//!             let root = div(class = "my-div") {
//!                 a(href = "http://zyghost.com") {
//!                     "Schellsan's website"
//!                 }
//!             }
//!         };
//!
//!         Self{ root }
//!     }
//! }
//! ```
//!
//! As you can see, the struct takes a type parameter `V:View` which has an
//! associated type `Element`. This allows us to write our views in a platform-agnostic
//! way, and then specialize at runtime:
//!
//! ```rust,no_run
//! use mogwai::web::Web;
//! # use mogwai::prelude::*;
//!
//! # struct Widget<V:View> {
//! #     root: V::Element
//! # }
//! # impl<V:View> Widget<V> {
//! #     fn new() -> Self {
//! #         rsx! {
//! #             let root = div(class = "my-div") {
//! #                 a(href = "http://zyghost.com") {
//! #                     "Schellsan's website"
//! #                 }
//! #             }
//! #         };
//!
//! #         Self{ root }
//! #     }
//! # }
//!
//! let web_element = Widget::<Web>::new();
//! ```
//!
//! [`Web`] is a type that implements [`View`]. It monomorphizes our struct
//! to produce a view using [`web_sys`](web_sys) types.
//!
//! Also provided is the [`Ssr`] type, which implements [`View`] to produce
//! views that render to [`String`] for server-side rendering.
//!
//! ```rust
//! use mogwai::ssr::Ssr;
//! # use mogwai::prelude::*;
//!
//! # struct Widget<V:View> {
//! #     root: V::Element
//! # }
//! # impl<V:View> Widget<V> {
//! #     fn new() -> Self {
//! #         rsx! {
//! #             let root = div(class = "my-div") {
//! #                 a(href = "http://zyghost.com") {
//! #                     "Schellsan's website"
//! #                 }
//! #             }
//! #         };
//!
//! #         Self{ root }
//! #     }
//! # }
//!
//! let ssr_element = Widget::<Ssr>::new();
//! println!("{}", ssr_element.root.html_string());
//! ```
//!
//! In this way server-side rendering is a separate view "platform" from the browser,
//! but we can build the view all the same using our `V:View` parameterization.
//!
//! [`View`] has a number of associated types:
//!
//! * **Element**
//! * **Text**
//! * **Node**
//! * **EventListener**
//! * **Event**
//!
//! They all work together to make your views cross-platform.
//!
//! ### Event handling
//!
//! The [`rsx!`] macro binds event listeners in attribute position to a name, which can
//! then be used by platform-agnostic logic:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct Widget<V:View> {
//!     root: V::Element,
//!     text: V::Text,
//!     on_click: V::EventListener,
//! }
//!
//! impl<V:View> Widget<V> {
//!     fn new() -> Self {
//!          rsx! {
//!              let root = div(class = "my-div") {
//!                  a(
//!                      // Here an event listener is registered and then bound to the name `on_click`
//!                      on:click = on_click,
//!                      href = "http://zyghost.com"
//!                  ) {
//!                      let text = "Schellsan's website"
//!                  }
//!              }
//!          };
//!
//!          Self{
//!              root,
//!              text,
//!              on_click,
//!          }
//!     }
//!
//!     async fn step(&self) {
//!         let _ev: V::Event = self.on_click.next().await;
//!         self.text.set_text("You clicked!");
//!     }
//! }
//! ```
//!
//! As you can see, the view platform is kept agnostic, and after the click event,
//! updating the text an obvious, intentional action on the part of the logic inside
//! the `step` function.
//!
//! ### Using [`Proxy`] for updates
//!
//! Views in Mogwai are dynamic, but they are updated explicitly in response to events.
//! Sometimes, though, we'd like to hold little bits of state in our views, and when
//! that state changes we want multiple parts of the view to "react".
//!
//! This doesn't violate Mogwai's goal of ensuring updates are explicit, in that the change must
//! be executed explicitly in logic, but the results of that change may occur in more
//! than one place, by use of the [`Proxy`] type.
//!
//! To use a [`Proxy`] we construct it outside of the [`rsx!`] macro and then use it
//! with some special notation inside the macro:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct Widget<V:View> {
//!     root: V::Element,
//!     on_click: V::EventListener,
//!     state: Proxy<u32>,
//! }
//!
//! impl<V:View> Widget<V> {
//!     fn new() -> Self {
//!          let mut state = Proxy::new(0);
//!
//!          rsx! {
//!              let root = div(class = "my-div") {
//!                  a(
//!                      // Here an event listener is registered and then bound to the name `on_click`
//!                      on:click = on_click,
//!                      href = "http://zyghost.com"
//!                  ) {
//!                      {state(n => match *n {
//!                          0 => "Schellsan's website",
//!                          _ => "You clicked!",
//!                      }.to_string())}
//!                      span() {
//!                          {state(n => format!("Counted {n} clicks."))}
//!                      }
//!                  }
//!              }
//!          };
//!
//!          Self{
//!              root,
//!              on_click,
//!              state,
//!          }
//!     }
//!
//!     async fn step(&mut self) {
//!         let _ev: V::Event = self.on_click.next().await;
//!         let current_clicks = *self.state;
//!         self.state.set(current_clicks + 1);
//!     }
//! }
//! ```
//!
//! In this example, clicking the button updates the state, which in turn updates
//! the paragraph text. The `Proxy` type is used to manage the state and trigger
//! updates to the view when the state changes.

#[allow(unused_imports)]
use super::prelude::*;
#[allow(unused_imports)]
use crate as mogwai;

//! An introduction to the minimal, obvious, graphical web application interface.
//!
//! # Welcome!
//! Mogwai is a cute little library for building browser interfaces. It is
//! cognitively small and runtime fast. It acheives these goals by doing very few
//! things, but doing those things well.
//!
//! The following is a short introduction to the concepts of Mogwai.
//!
//! ## Building Gizmos (aka constructing DOM widgets)
//! Building DOM is one of the main authorship modes in Mogwai. DOM nodes
//! are created using a builder pattern. The builder itself is called
//! [`GizmoBuilder`] and it gets built into a [`Gizmo`]. A `Gizmo` is Mogwai's
//! name for a widget. `Gizmo`s can be run or attached to other `Gizmo`s. The
//! builder pattern looks like this:
//!
//! ```rust, no_run
//! extern crate mogwai;
//! use::mogwai::prelude::*;
//!
//! div()
//!   .class("my-div")
//!   .with(
//!     a()
//!       .attribute("href", "http://zyghost.com")
//!       .text("Schellsan's website")
//!   )
//!   .build().unwrap_throw()
//!   .run().unwrap_throw()
//! ```
//!
//! The example above would create a DOM node and append it to the document
//! body. It would look like this:
//!
//! ```html
//! <div class="my-div">
//!   <a href="http://zyghost.com">Schell's website</a>
//! </div>
//! ```
//!
//! ### Running Gizmos and removing gizmos
//!
//! Note that the builder is built into a [`Gizmo`] with [`GizmoBuilder::build`]
//! and then added to the body automatically with the [`GizmoBuilder::run`]
//! function. This `run` function is special. It hands off the gizmo to be
//! owned by the window. Otherwise the gizmo would go out of scope and be
//! dropped. When a gizmo is dropped its [`HtmlElement`] is removed from the DOM.
//!
//! You may have noticed that we can use the [`GizmoBuilder::class`] method to set
//! the class of our `div` tag, but we use the [`GizmoBuilder::attribute`] method
//! to set the href attribute of our `a` tag. That's because [`GizmoBuilder::class`]
//! is a convenience method that simply calls [`GizmoBuilder::attribute`] under the
//! hood. Some DOM attributes have these conveniences and others do not. See the
//! documentation for [`GizmoBuilder`] for more info. If you don't see a method that
//! you think should be there, I welcome you to
//! [add it in a pull request](https://github.com/schell/mogwai) :)
//!
//! ### Wiring DOM
//!
//! `Gizmo`s can be static like the one above, or they can change over time.
//! `Gizmo`s get their dynamic values from the receiving end of a channel
//! called a [`Receiver<T>`]. The transmitting end of the channel is called a
//! [`Transmitter<T>`]. This should be somewhat familiar if you've ever used a
//! channel in other rust libraries.
//!
//! Creating a channel is easy using the [txrx()] function. Then we "wire" it
//! into the `GizmoBuilder` with one of a number of `rx_` flavored `GizmoBuilder`
//! methods.
//!
//! Whenever the `Transmitter<T>` of the channel sends a value, the DOM is
//! updated.
//!
//! ```rust, no_run
//! extern crate mogwai;
//! use::mogwai::prelude::*;
//!
//! let (tx, rx) = txrx();
//!
//! div()
//!   .class("my-div")
//!   .with(
//!     a()
//!       .attribute("href", "https://zyghost.com")
//!       .rx_text("Schellsan's website", rx)
//!   )
//!   .build().unwrap_throw()
//!   .run().unwrap_throw();
//!
//! tx.send(&"Gizmo's website".into());
//! ```
//!
//! Just like previously, this builds a DOM node and appends it to the document
//! body, but this time we've already updated the link's text to "Gizmo's website":
//!
//! ```html
//! <div class="my-div">
//!   <a href="http://zyghost.com">Gizmo's website</a>
//! </div>
//! ```
//!
//! ### Components and more advanced wiring
//!
//! In bigger applications we often have circular dependencies between buttons,
//! fields and other interface elements. When these complex situations arise
//! we compartmentalize concerns into [`Component`]s.
//!
//! Other times we don't need a full component with an update cycle and instead
//! we simply require
//! [transmitters, receivers and some handy folds and maps](super::txrx).
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
//!
//! [`GizmoBuilder::build`]: GizmoBuilder::build
//! [`GizmoBuilder::run`]: GizmoBuilder::method@run
//! [`GizmoBuilder`]: struct@GizmoBuilder
//! [`Gizmo`]: struct@Gizmo
//! [`Transmitter<T>`]: struct@Transmitter
//! [`Receiver<T>`]: struct@Receiver
//! [`HtmlElement`]: struct@HtmlElement
//! [`Component`]: trait@Component
use super::gizmo::*;
use super::txrx::*;
use super::component::*;
use super::component::subscriber::*;


struct Unit {}

impl Component for Unit {
  type ModelMsg = ();
  type ViewMsg = ();
  type DomNode = Element;

  fn view(&self, _: Transmitter<()>, _: Receiver<()>) -> Gizmo<Element> {
    Gizmo::element("") as Gizmo<Element>
  }
  fn update(&mut self, _: &(), _: &Transmitter<()>, _sub: &Subscriber<()>) {}
}

// This is here just for the documentation links.
fn _not_used() {
  let _element = Gizmo::element("");
  let (_tx, _rx) = txrx::<()>();
}

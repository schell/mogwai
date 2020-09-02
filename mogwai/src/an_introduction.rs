//! An introduction to the minimal, obvious, graphical web application interface.
//!
//! # Welcome!
//! Mogwai is a cute little library for building browser interfaces. It is
//! cognitively small and runtime fast. It acheives these goals by doing very few
//! things, but doing those things well.
//!
//! The following is a short introduction to the concepts of Mogwai.
//!
//! ## Constructing DOM Nodes
//! Building DOM is one of the main tasks of web development. In mogwai the
//! quickest way to construct DOM nodes is with the [`dom`] RSX macro. RSX
//! is a lot like React's JSX, except that it uses type checket rust expressions.
//! The [`dom`] macro evaluates to a [`View`] which is used as the view
//! of a [`Component`] or can be used by itself:
//!
//! ```rust, no_run
//! extern crate mogwai;
//! use::mogwai::prelude::*;
//!
//! view!(
//!     <div class="my-div">
//!         <a href="http://zyghost.com">
//!             "Schellsan's website"
//!         </a>
//!     </div>
//!   ).run().unwrap_throw()
//! ```
//!
//! The example above would create a DOM node with a link inside it and append it
//! to the document body. It would look like this:
//!
//! ```html
//! <div class="my-div">
//!       <a href="http://zyghost.com">Schell's website</a>
//! </div>
//! ```
//!
//! ### Running Gizmos and removing gizmos
//!
//! Note that by using the [`View::run`] function the nod is added to the
//! body automatically. This `run` function is special. It hands off the
//! callee to be *owned by the window* - otherwise the gizmo would go out of scope
//! and be dropped. This is important - when a [`View`] is dropped and all references
//! to its inner DOM node are no longer in scope, that DOM node is removed from
//! the DOM.
//!
//! ### Wiring DOM
//!
//! [`View`]s can be static like the one above, or they can change over time.
//! [`View`]s get their dynamic values from the receiving end of a channel
//! called a [`Receiver<T>`]. The transmitting end of the channel is called a
//! [`Transmitter<T>`]. This should be somewhat familiar if you've ever used a
//! channel in other rust libraries.
//!
//! You can create a channel using the [txrx()] function. Then "wire" it
//! into the DOM using RSX - simply assign it to an attribute or add
//! it as a text node. You may even tuple it with an initial value.
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
//! view!(
//!     <div class="my-div">
//!         <a href="http://zyghost.com">
//!           {("Schellsan's website", rx)}
//!         </a>
//!     </div>
//! ).run().unwrap_throw();
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
//! [`dom`]: dom
//! [`View::run`]: View::method@run
//! [`View`]: struct@View
//! [`View`]: struct@View
//! [`Transmitter<T>`]: struct@Transmitter
//! [`Receiver<T>`]: struct@Receiver
//! [`HtmlElement`]: struct@HtmlElement
//! [`Component`]: trait@Component
use super::prelude::*;
use crate as mogwai;


struct Unit {}

impl Component for Unit {
    type ModelMsg = ();
    type ViewMsg = ();
    type DomNode = HtmlElement;

    fn view(&self, _: Transmitter<()>, _: Receiver<()>) -> View<HtmlElement> {
        view! {
            <a href="/#">"Hello"</a>
        }
    }
    fn update(&mut self, _: &(), _: &Transmitter<()>, _sub: &Subscriber<()>) {}
}

// This is here just for the documentation links.
fn _not_used() {
    let (_tx, _rx) = txrx::<()>();
}

#![allow(unused_braces)]
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
//!
//! Building DOM is one of the main tasks of web development. In mogwai the
//! quickest way to construct DOM nodes is by using one of the built in RSX macros.
//!
//! `builder!` or `view!` are flavors of mogwai's RSX that evaluate to
//! [`ViewBuilder`] or [`View`] respectively. RSX is a lot like react.js's JSX,
//! except that it uses type checked rust expressions.
//!
//! Most of the time you'll see the [`builder!`] macro used to create a [`ViewBuilder`]:
//!
//! ```rust
//! # extern crate mogwai;
//! use::mogwai::prelude::*;
//!
//! let my_div: ViewBuilder<HtmlElement> = builder!(
//!     <div class="my-div">
//!         <a href="http://zyghost.com">
//!             "Schellsan's website"
//!         </a>
//!     </div>
//!   );
//! ```
//!
//! The [`ViewBuilder`] can be converted into a [`View`]:
//!
//! ```rust
//! # extern crate mogwai;
//! # use::mogwai::prelude::*;
//!
//! # let my_div: ViewBuilder<HtmlElement> = builder!(
//! #     <div class="my-div">
//! #         <a href="http://zyghost.com">
//! #             "Schellsan's website"
//! #         </a>
//! #     </div>
//! #   );
//! let view: View<HtmlElement> = View::from(my_div);
//!
//! assert_eq!(
//!     view.html_string(),
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
//! # extern crate mogwai;
//! # use::mogwai::prelude::*;
//!
//! # let my_div: ViewBuilder<HtmlElement> = builder!(
//! #     <div class="my-div">
//! #         <a href="http://zyghost.com">
//! #             "Schellsan's website"
//! #         </a>
//! #     </div>
//! #   );
//! # let view: View<HtmlElement> = View::from(my_div);
//! view.run().unwrap();
//! ```
//!
//! [`View::run`] consumes the view, *handing ownership to the browser window*.
//! Because of this it is only available on `wasm32`.
//!
//! ### Dropping a [`View`]
//!
//! By handing the [`View`] off to the window it never goes out of scope.
//! This is important - when a [`View`] is dropped and all references
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
//! ```rust
//! # extern crate mogwai;
//! use::mogwai::prelude::*;
//!
//! let (tx, rx) = txrx();
//!
//! let my_view = view!(
//!     <div class="my-div">
//!         <a href="http://zyghost.com">
//!             // start with a value and update when a message
//!             // is received on rx.
//!             {("Schellsan's website", rx)}
//!         </a>
//!     </div>
//! );
//!
//! tx.send(&"Gizmo's website".into());
//! ```
//!
//! A [`Transmitter`] can be used to send DOM events as messages, allowing
//! your view to communicate with itself or other components:
//! ```rust
//! # extern crate mogwai;
//! use::mogwai::prelude::*;
//!
//! let (tx, rx) = txrx();
//!
//! let my_view = view!(
//!     <div class="my-div">
//!         <a href="#" on:click=tx.contra_map(|_: &Event| "Gizmo's website".to_string())>
//!             // start with a value and update when a message
//!             // is received on rx.
//!             {("Schellsan's website", rx)}
//!         </a>
//!     </div>
//! );
//! ```
//!
//! See [txrx's module level documentation](super::txrx) for more info on mapping
//! `Transmitter`s and `Receiver`s.
//!
//! ### Accessing the underlying DOM node
//!
//! The [`View`] contains a reference to the raw DOM node, making it possible
//! to manipulate the DOM by hand using Javascript FFI bindings and functions
//! provided by the great `web_sys` crate:
//!
//! ```rust, no_run
//! # extern crate mogwai;
//! # use::mogwai::prelude::*;
//!
//! # let (tx, rx) = txrx();
//!
//! # let my_view = view!(
//! #     <div class="my-div">
//! #         <a href="http://zyghost.com">
//! #             // start with a value and update when a message
//! #             // is received on rx.
//! #             {("Schellsan's website", rx)}
//! #         </a>
//! #     </div>
//! # );
//! # tx.send(&"Gizmo's website".into());
//! #
//! let node: std::cell::Ref<HtmlElement> = my_view.dom_ref();
//! assert_eq!(
//!     node.inner_html(),
//!     r#"<a href="http://zyghost.com">Gizmo's website</a>"#
//! );
//! ```
//!
//! ### Components and more advanced wiring
//!
//! For anything but the simplest view, it is recommended you use the
//! [`Component`] trait to build your view components.
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
//! [`builder!`]: builder
//! [`view!`]: view
//! [`View::run`]: View::method@run
//! [`View`]: struct@View
//! [`ViewBuilder`]: struct@ViewBuilder
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

    fn view(&self, _: &Transmitter<()>, _: &Receiver<()>) -> ViewBuilder<HtmlElement> {
        builder! {
            <a href="/#">"Hello"</a>
        }
    }
    fn update(&mut self, _: &(), _: &Transmitter<()>, _sub: &Subscriber<()>) {}
}

// This is here just for the documentation links.
fn _not_used() {
    let (_tx, _rx) = txrx::<()>();
}

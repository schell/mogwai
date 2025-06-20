#![allow(unused_braces)]
//! # Welcome!
//!
//! This is an introduction to writing user interfaces with the mogwai crate.
//!
//! <div align="center">
//!   <h1>
//!     <img src="https://raw.githubusercontent.com/schell/mogwai/master/img/gizmo.svg" />
//!     <br />
//!     mogwai
//!   </h1>
//! </div>
//!
//! The following is a short introduction to the library's basic concepts.
//!
//! ## Preludes
//!
//! There are a _few_ prelude modules that you can glob-import to make development easier:
//!
//! The first is the common prelude, which contains cross-platform types and traits.
//!
//! ```rust
//! use mogwai::prelude::*;
//! ```
//!
//! Then there are more domain-specific preludes for web and server-side rendering,
//! both of which re-export the common prelude:
//!
//! ```rust
//! use mogwai::web::prelude::*;
//! use mogwai::ssr::prelude::*;
//! ```
//!
//! The [web prelude](crate::web::prelude) also re-exports a few of the most commonly
//! used WASM crates as a convenience, such as [`web_sys`], [`wasm_bindgen`] and
//! [`wasm_bindgen_futures`].
//!
//! ## View Construction
//!
//! View construction is accomplished using a novel [`rsx!`] macro that reduces
//! boilerplate and has special syntax for setting node attributes, text and nesting
//! views.
//!
//! ### RSX
//!
//! [`rsx!`] is a lot like react.js's JSX, except that it uses type checked Rust expressions.
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
//! # use mogwai::web::Web;
//! # use mogwai::prelude::*;
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
//! #         Self{ root }
//! #     }
//! # }
//! let web_element = Widget::<Web>::new();
//! ```
//!
//! [`Web`](crate::web::Web) is a type that implements [`View`]. It monomorphizes our struct
//! to produce a view using [`web_sys`] types.
//!
//! Also provided is the [`Ssr`](crate::ssr::Ssr) type, which implements [`View`] to produce
//! views that render to [`String`] for server-side rendering.
//!
//! ```rust
//! # use mogwai::ssr::Ssr;
//! # use mogwai::prelude::*;
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
//! #         Self{ root }
//! #     }
//! # }
//! let ssr_element = Widget::<Ssr>::new();
//! println!("{}", ssr_element.root.html_string());
//! ```
//!
//! In this way, server-side rendering is a separate view "platform" from the browser,
//! but we can build the view all the same using our `V:View` parameterization.
//!
//! ### Cross-platform
//!
//! [`View`] has a number of associated types:
//!
//! * **Element**
//! * **Text**
//! * **Node**
//! * **EventListener**
//! * **Event**
//!
//! They all work together with few [interlocking traits](crate::view#traits)
//! to make your views cross-platform. But you can also specialize certain
//! operations to specific platforms using the convenience functions
//! [`when_element`](crate::view::ViewElement::when_element)
//! and [`when_event`](crate::view::ViewEvent::when_event):
//!
//! ```rust                                                                                        
//! use mogwai::web::prelude::*;                                                                        
//!                                                                                                
//! struct MyView<V: View> {                                                                       
//!     root: V::Element,                                                                          
//!     button: V::Element,
//! }                                                                                              
//!                                                                                                
//! impl<V: View> MyView<V> {                                                                      
//!     fn new() -> Self {                                                                         
//!         rsx! {                                                                                 
//!             let root = div(class = "my-view") {                                                
//!                 h1() { "Hello, Mogwai!" }                                                      
//!                 let button = button() {                           
//!                     "Click me"                                                                 
//!                 }                                                                              
//!             }                                                                                  
//!         }                                                                                      
//!         Self { root, button }                                                                          
//!     }                                                                                          
//!                                                                                                
//!     fn specialize_for_web(&self) {                                                             
//!         self.button.when_element::<Web, _>(|el: &web_sys::Element| {                                                
//!             el.set_property("data-special", "web");                                            
//!         });                                                                                    
//!     }                                                                                          
//! }                                                                                              
//! ```
//!
//! We can even go a step further when specializing for the web by using
//! the [`dyn_el`](crate::web::WebElement::dyn_el) and
//! [`dyn_ev`](crate::web::WebEvent::dyn_ev) extension methods, which cast your
//! elements and events to specific [`web_sys`] types that implement
//! [`JsCast`](wasm_bindgen::JsCast).
//!
//! ```rust
//! use mogwai::web::prelude::*;                                                                        
//!                                                                                                
//! struct MyView<V: View> {                                                                       
//!     root: V::Element,                                                                          
//!     input: V::Element,
//! }                                                                                              
//!                                                                                                
//! impl<V: View> MyView<V> {                                                                      
//!     fn new() -> Self {                                                                         
//!         rsx! {                                                                                 
//!             let root = div(class = "my-view") {                                                
//!                 h1() { "Use the input:" }                                                      
//!                 let input = input(type_ = "text") {}                                                                              
//!             }                                                                                  
//!         }                                                                                      
//!         Self { root, input }                                                                          
//!     }                                                                                          
//!                                                                                                
//!     fn specialize_for_web(&self) {                                                             
//!         self.input.dyn_el(|input: &web_sys::HtmlInputElement| {                                                
//!             let value = input.value();
//!             // do special stuff with the input value here...
//!         });                                                                                    
//!     }                                                                                          
//! }
//! ```
//!
//! ### Event handling
//!
//! The [`rsx!`] macro binds [event listeners](crate::view::ViewEventListener) in
//! attribute position to a name, which can then be used by platform-agnostic
//! logic:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct Widget<V:View> {
//!     root: V::Element,
//!     text: V::Text,
//!     /// A cross-platform event listener, which responds to `.next()` to await the
//!     /// next event occurence.
//!     on_click: V::EventListener,
//! }
//!
//! impl<V:View> Widget<V> {
//!     fn new() -> Self {
//!          rsx! {
//!              let root = div(class = "my-div") {
//!                  a(
//!                      // Here an event listener is registered and then bound
//!                      // to the name `on_click`
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
//! updating the text is an obvious, intentional action on the part of the logic inside
//! the `step` function.
//!
//! ### Using [`Proxy`] for updates
//!
//! Views in mogwai are dynamic, but they are updated explicitly in response to events.
//! Sometimes, though, we'd like to hold little bits of state in our views, and when
//! that state changes we want multiple parts of the view to "react".
//!
//! This doesn't violate mogwai's goal of ensuring updates are explicit, in that the change must
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
//!
//! #### [`Proxy`] API notes
//!
//! Note that [`Proxy`] is not `Clone`, and that modifying a [`Proxy`] requires mutation.
//! This is a purposeful design choice to make tracking down data updates easy.
//!
//! # Getting started
//!
//! That's it! Time to get started. If you're looking for a project template you can use
//! the [`cargo-generate`](https://crates.io/crates/cargo-generate)
//! [`mogwai-template`](https://github.com/schell/mogwai-template).

#[allow(unused_imports)]
use super::prelude::*;
#[allow(unused_imports)]
use crate as mogwai;

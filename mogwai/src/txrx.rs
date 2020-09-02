//! # Instant channels.
//!
//! Just add water! ;)
//!
//! ## Creating channels
//! There are a number of ways to create a channel in this module. The most
//! straight forward is to use the function [txrx]. This will create a linked
//! [Transmitter] + [Receiver] pair:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx, rx): (Transmitter<()>, Receiver<()>) = txrx();
//! ```
//!
//! Or maybe you prefer an alternative syntax:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx, rx) = txrx::<()>();
//! ```
//!
//! Or simply let the compiler try to figure it out:
//!
//! ```rust, ignore
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx, rx) = txrx();
//! // ...
//! ```
//!
//! This pair makes a linked channel. Messages you send on the [Transmitter]
//! will be sent directly to the [Receiver] on the other end.
//!
//! You can create separate terminals using the [trns] and [recv] functions. Then
//! later in your code you can spawn new linked partners from them:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let mut tx = trns();
//! let rx = tx.spawn_recv();
//! tx.send(&()); // rx will receive the message
//! ```
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let rx = recv();
//! let tx = rx.new_trns();
//! tx.send(&()); // rx will receive the message
//! ```
//!
//! Note that [Transmitter::spawn_recv] mutates the transmitter its called on,
//! while [Receiver::new_trns] requires no such mutation.
//!
//! ## Sending messages
//!
//! Once you have a txrx pair you can start sending messages:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx, rx) = txrx();
//! tx.send(&());
//! tx.send(&());
//! tx.send(&());
//! ```
//!
//! Notice that we send references. This is because neither the transmitter nor
//! the receiver own the messages.
//!
//! It's also possible to send asynchronous messages! We can do this with
//! [Transmitter::send_async], which takes a type that implements [Future]. Here
//! is an example of running an async web request to send some text from an
//! `async` block:
//!
//! ```rust, no_run
//! extern crate mogwai;
//! extern crate web_sys;
//! use mogwai::prelude::*;
//! use web_sys::{Request, RequestMode, RequestInit, Response};
//!
//! // Here's our async function that fetches a text response from a server,
//! // or returns an error string.
//! async fn request_to_text(req:Request) -> Result<String, String> {
//!   let resp:Response =
//!     JsFuture::from(
//!       window()
//!         .fetch_with_request(&req)
//!     )
//!     .await
//!     .map_err(|_| "request failed".to_string())?
//!     .dyn_into()
//!     .map_err(|_| "response is malformed")?;
//!   let text:String =
//!     JsFuture::from(
//!       resp
//!         .text()
//!         .map_err(|_| "could not get response text")?
//!     )
//!     .await
//!     .map_err(|_| "getting text failed")?
//!     .as_string()
//!     .ok_or("couldn't get text as string".to_string())?;
//!   Ok(text)
//! }
//!
//! let (tx, rx) = txrx();
//! tx.send_async(async {
//!   let mut opts = RequestInit::new();
//!   opts.method("GET");
//!   opts.mode(RequestMode::Cors);
//!
//!   let req =
//!     Request::new_with_str_and_init(
//!       "https://worldtimeapi.org/api/timezone/Europe/London.txt",
//!       &opts
//!     )
//!     .unwrap_throw();
//!
//!   request_to_text(req)
//!     .await
//!     .unwrap_or_else(|e| e)
//! });
//! ```
//!
//! ## Responding to messages
//!
//! [Receiver]s can respond immediately to the messages that are sent to them.
//! There is no polling and no internal message buffer. These channels are
//! instant! Receivers do this by invoking their response function when they
//! receive a message. The response function can be set using
//! [Receiver::respond]:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx, rx) = txrx();
//! rx.respond(|&()| {
//!     println!("Message received!");
//! });
//! tx.send(&());
//! ```
//!
//! For convenience we also have the [Receiver::respond_shared] method and the
//! [new_shared] function that together allow you to respond using a shared
//! mutable variable. Inside your fold function you can simply mutate this shared
//! variable as normal. This makes it easy to encapsulate a little bit of shared
//! state in your responder without requiring much knowledge about thread-safe
//! asynchronous programming:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let shared_count = new_shared(0);
//! let (tx, rx) = txrx();
//! rx.respond_shared(shared_count.clone(), |count: &mut i32, &()| {
//!     *count += 1;
//!     println!("{} messages received!", *count);
//! });
//! tx.send(&());
//! tx.send(&());
//! tx.send(&());
//! assert_eq!(*shared_count.borrow(), 3);
//! ```
//!
//! ## Composing channels
//!
//! Sending messages into a transmitter and having it pop out automatically is
//! great, but wait, there's more! What if we have a `tx_a:Transmitter<A>` and a
//! `rx_b:Receiver<B>`, but we want to send `A`s on `tx_a` and have `B`s pop out
//! of `rx_b`? We could use the machinery we have and write something like:
//!
//! ```rust, ignore
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx_a, rx_b) = {
//!   let (tx_a, rx_a) = txrx();
//!   let (tx_b, rx_b) = txrx();
//!   let f = |a| { a.turn_into_b() };
//!   rx_a.respond(move |a| {
//!     tx_b.send(f(a));
//!   });
//!   (tx_a, rx_b)
//! };
//! ```
//!
//! And indeed, it works! But that's an awful lot of boilerplate just to get a
//! channel of `A`s to `B`s. Instead we can use the `txrx_map` function, which
//! does all of this for us given the map function. Here's an example using
//! a `Transmitter<()>` that sends to a `Receiver<i32>`:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! // For every unit that gets sent, map it to `1:i32`.
//! let (tx_a, rx_b) = txrx_map(|&()| 1);
//! let shared_count = new_shared(0);
//! rx_b.respond_shared(shared_count.clone(), |count: &mut i32, n: &i32| {
//!     *count += n;
//!     println!("Current count is {}", *count);
//! });
//!
//! tx_a.send(&());
//! tx_a.send(&());
//! tx_a.send(&());
//! assert_eq!(*shared_count.borrow(), 3);
//! ```
//!
//! That is useful, but we can also do much more than simple maps! We can fold
//! over an internal state or a shared state, we can filter some of the sent
//! messages and we can do all those things together! Check out the `txrx_*`
//! family of functions:
//!
//! * [txrx]
//! * [txrx_filter_fold]
//! * [txrx_filter_fold_shared]
//! * [txrx_filter_map]
//! * [txrx_fold]
//! * [txrx_fold_shared]
//! * [txrx_map]
//!
//! You'll also find functions with these flavors in [Transmitter] and
//! [Receiver].
//!
//! ## Wiring [Transmitter]s and forwading [Receiver]s
//!
//! Another way to get a txrx pair of different types is to create each side
//! separately using [trns] and [recv] and then wire them together:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let mut tx = trns::<()>();
//! let rx = recv::<i32>();
//! tx.wire_map(&rx, |&()| 1);
//! ```
//!
//! The following make up the `wire_*` family of functions on [Transmitter]:
//!
//! * [Transmitter::wire_filter_fold]
//! * [Transmitter::wire_filter_fold_async]
//! * [Transmitter::wire_filter_fold_shared]
//! * [Transmitter::wire_filter_map]
//! * [Transmitter::wire_fold]
//! * [Transmitter::wire_fold_shared]
//! * [Transmitter::wire_map]
//!
//! Note that they all mutate the [Transmitter] they are called on.
//!
//! Conversely, if you would like to forward messages from a receiver into a
//! transmitter of a different type you can "forward" messages from the receiver
//! to the transmitter:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx, rx) = txrx::<()>();
//! let (mut tx_i32, rx_i32) = txrx::<i32>();
//! rx.forward_map(&tx_i32, |&()| 1);
//!
//! let shared_got_it = new_shared(false);
//! rx_i32.respond_shared(shared_got_it.clone(), |got_it: &mut bool, n: &i32| {
//!     println!("Got {}", *n);
//!     *got_it = true;
//! });
//!
//! tx.send(&());
//! assert_eq!(*shared_got_it.borrow(), true);
//! ```
//!
//! These make up the `forward_*` family of functions on [Receiver]:
//!
//! * [Receiver::forward_filter_fold]
//! * [Receiver::forward_filter_fold_async]
//! * [Receiver::forward_filter_fold_shared]
//! * [Receiver::forward_filter_map]
//! * [Receiver::forward_fold]
//! * [Receiver::forward_fold_shared]
//! * [Receiver::forward_map]
//!
//! Note that they all consume the [Receiver] they are called on.
//!
//! ## Cloning, branching, etc
//!
//! [Transmitter]s may be cloned. Once a transmitter is cloned a message sent on
//! either the clone or the original will pop out on any linked receivers:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx1, rx) = txrx();
//! let tx2 = tx1.clone();
//! let shared_count = new_shared(0);
//! rx.respond_shared(shared_count.clone(), |count: &mut i32, &()| {
//!     *count += 1;
//! });
//! tx1.send(&());
//! tx2.send(&());
//! assert_eq!(*shared_count.borrow(), 2);
//! ```
//!
//! [Receiver]s are a bit different from [Transmitter]s, though. They are _not_
//! clonable because they house a responder, which must be unique. Instead we can
//! use [Receiver::branch] to create a new receiver that is linked to the same
//! transmitters as the original, but owns its own unique response to messages:
//!
//! ```rust
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! let (tx, rx1) = txrx();
//! let rx2 = rx1.branch();
//! let shared_count = new_shared(0);
//! rx1.respond_shared(shared_count.clone(), |count: &mut i32, &()| {
//!     *count += 1;
//! });
//! rx2.respond_shared(shared_count.clone(), |count: &mut i32, &()| {
//!     *count += 1;
//! });
//! tx.send(&());
//! assert_eq!(*shared_count.borrow(), 2);
//! ```
//!
//! Both [Transmitter]s and [Receiver]s can be "branched" so that multiple
//! transmitters may send to the same receiver and multiple receivers may respond
//! to the same transmitter. These use the `contra_*` family of functions on
//! [Transmitter] and the `branch_*` family of functions on [Receiver].
//!
//! ### Transmitter's contra_* family
//!
//! This family of functions are named after Haskell's [contramap]. That's
//! because these functions take a transmitter of `B`s, some flavor of function
//! that transforms `B`s into `A`s and returns a new transmitter of `A`s.
//! Essentially - the newly created transmitter extends the original _backward_,
//! allowing you to send `A`s into it and have `B`s automatically sent on the
//! original.
//!
//! * [Transmitter::contra_filter_fold]
//! * [Transmitter::contra_filter_fold_shared]
//! * [Transmitter::contra_filter_map]
//! * [Transmitter::contra_fold]
//! * [Transmitter::contra_map]
//!
//! ### Receiver's branch_* family
//!
//! This family of functions all extend new receivers off of an original and
//! can transform messages of `A`s received on the original into messages of `B`s
//! received on the newly created receiver. This is analogous to Haskell's
//! [fmap].
//!
//! * [Receiver::branch]
//! * [Receiver::branch_filter_fold]
//! * [Receiver::branch_filter_fold_shared]
//! * [Receiver::branch_filter_map]
//! * [Receiver::branch_fold]
//! * [Receiver::branch_fold_shared]
//! * [Receiver::branch_map]
//!
//! ### [Receiver::merge]
//!
//! If you have many receivers that you would like to merge you can use the
//! [Receiver::merge] function.
//!
//! ## Done!
//!
//! The channels defined here are the backbone of this library. Getting to
//! know the many constructors and combinators may seem like a daunting task but
//! don't worry - the patterns of branching, mapping and folding are functional
//! programming's bread and butter. Once you get a taste for this flavor of
//! development you'll want more (and it will get easier). But remember,
//! no matter how much it begs, no matter how much it cries, [NEVER feed Mogwai
//! after midnight](https://youtu.be/OrHdo-v9mRA) ;)
//!
//! [contramap]: https://hackage.haskell.org/package/base-4.12.0.0/docs/Data-Functor-Contravariant.html#v:contramap
//! [fmap]: https://hackage.haskell.org/package/base-4.12.0.0/docs/Data-Functor.html#v:fmap
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    future::Future,
    pin::Pin,
    rc::Rc,
};
use wasm_bindgen_futures::spawn_local;

pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<A>>>>;


pub fn wrap_future<A, F>(future: F) -> Option<RecvFuture<A>>
where
    F: Future<Output = Option<A>> + 'static,
{
    Some(Box::pin(future))
}


struct Responders<A> {
    next_k: Cell<usize>,
    branches: RefCell<HashMap<usize, Box<dyn FnMut(&A)>>>,
}

impl<A> Default for Responders<A> {
    fn default() -> Self {
        Self {
            next_k: Cell::new(0),
            branches: Default::default(),
        }
    }
}

impl<A> Responders<A> {
    fn recv_from(self: Rc<Self>) -> Receiver<A> {
        let k = {
            let k = self.next_k.get();
            self.next_k.set(k + 1);
            k
        };

        Receiver {
            k,
            responders: self,
        }
    }

    fn insert(&self, k: usize, f: impl FnMut(&A) + 'static) {
        self.branches.borrow_mut().insert(k, Box::new(f));
    }

    fn remove(&self, k: usize) {
        self.branches.borrow_mut().remove(&k);
    }

    fn send(&self, a: &A) {
        self.branches.borrow_mut().values_mut().for_each(|f| {
            f(a);
        });
    }
}


/// Send messages instantly.
pub struct Transmitter<A> {
    responders: Rc<Responders<A>>,
}


impl<A> Clone for Transmitter<A> {
    fn clone(&self) -> Self {
        Self {
            responders: self.responders.clone(),
        }
    }
}


impl<A: 'static> Transmitter<A> {
    /// Create a new transmitter.
    pub fn new() -> Transmitter<A> {
        Self {
            responders: Default::default(),
        }
    }

    /// Spawn a receiver for this transmitter.
    pub fn spawn_recv(&self) -> Receiver<A> {
        self.responders.clone().recv_from()
    }

    /// Send a message to any and all receivers of this transmitter.
    pub fn send(&self, a: &A) {
        self.responders.send(a);
    }

    /// Execute a future that results in a message, then send it.
    pub fn send_async<FutureA>(&self, fa: FutureA)
    where
        FutureA: Future<Output = A> + 'static,
    {
        let tx = self.clone();
        spawn_local(async move {
            let a: A = fa.await;
            tx.send(&a);
        });
    }

    /// Extend this transmitter with a new transmitter using a filtering fold
    /// function. The given function folds messages of `B` over a shared state `T`
    /// and optionally sends `A`s down into this transmitter.
    pub fn contra_filter_fold_shared<B, T, F>(&self, var: Rc<RefCell<T>>, f: F) -> Transmitter<B>
    where
        B: 'static,
        T: 'static,
        F: Fn(&mut T, &B) -> Option<A> + 'static,
    {
        let tx = self.clone();
        let (tev, rev) = txrx();
        rev.respond(move |ev| {
            let result = {
                let mut t = var.borrow_mut();
                f(&mut t, ev)
            };
            result.into_iter().for_each(|b| {
                tx.send(&b);
            });
        });
        tev
    }

    /// Extend this transmitter with a new transmitter using a filtering fold
    /// function. The given function folds messages of `B` over a state `T` and
    /// optionally sends `A`s into this transmitter.
    pub fn contra_filter_fold<B, X, T, F>(&self, init: X, f: F) -> Transmitter<B>
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &B) -> Option<A> + 'static,
    {
        let tx = self.clone();
        let (tev, rev) = txrx();
        let mut t = init.into();
        rev.respond(move |ev| {
            f(&mut t, ev).into_iter().for_each(|b| {
                tx.send(&b);
            });
        });
        tev
    }

    /// Extend this transmitter with a new transmitter using a fold function.
    /// The given function folds messages of `B` into a state `T` and sends `A`s
    /// into this transmitter.
    pub fn contra_fold<B, X, T, F>(&self, init: X, f: F) -> Transmitter<B>
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &B) -> A + 'static,
    {
        self.contra_filter_fold(init, move |t, ev| Some(f(t, ev)))
    }

    /// Extend this transmitter with a new transmitter using a filter function.
    /// The given function maps messages of `B` and optionally sends `A`s into this
    /// transmitter.
    pub fn contra_filter_map<B, F>(&self, f: F) -> Transmitter<B>
    where
        B: 'static,
        F: Fn(&B) -> Option<A> + 'static,
    {
        self.contra_filter_fold((), move |&mut (), ev| f(ev))
    }

    /// Extend this transmitter with a new transmitter using a map function.
    /// The given function maps messages of `B` into `A`s and sends them all into
    /// this transmitter. This is much like Haskell's
    /// [contramap](https://hackage.haskell.org/package/base-4.12.0.0/docs/Data-Functor-Contravariant.html#v:contramap),
    /// hence the `contra_` prefix on this family of methods.
    pub fn contra_map<B, F>(&self, f: F) -> Transmitter<B>
    where
        B: 'static,
        F: Fn(&B) -> A + 'static,
    {
        self.contra_filter_map(move |ev| Some(f(ev)))
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function, where the state is a shared mutex.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the receiver.
    pub fn wire_filter_fold_shared<T, B, F>(&self, rb: &Receiver<B>, var: Rc<RefCell<T>>, f: F)
    where
        B: 'static,
        T: 'static,
        F: Fn(&mut T, &A) -> Option<B> + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold_shared(&tb, var, f);
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the receiver.
    pub fn wire_filter_fold<T, B, X, F>(&self, rb: &Receiver<B>, init: X, f: F)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<B> + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold(&tb, init, f);
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function.
    pub fn wire_fold<T, B, X, F>(&self, rb: &Receiver<B>, init: X, f: F)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> B + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_fold(&tb, init, f);
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function, where the state is a shared mutex.
    pub fn wire_fold_shared<T, B, F>(&self, rb: &Receiver<B>, var: Rc<RefCell<T>>, f: F)
    where
        B: 'static,
        T: 'static,
        F: Fn(&mut T, &A) -> B + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold_shared(&tb, var, move |t, a| Some(f(t, a)));
    }

    /// Wires the transmitter to the given receiver using a stateless map function.
    /// If the map function returns `None` for any messages those messages will
    /// *not* be sent to the given transmitter.
    pub fn wire_filter_map<B, F>(&self, rb: &Receiver<B>, f: F)
    where
        B: 'static,
        F: Fn(&A) -> Option<B> + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_map(&tb, f);
    }

    /// Wires the transmitter to the given receiver using a stateless map function.
    pub fn wire_map<B, F>(&self, rb: &Receiver<B>, f: F)
    where
        B: 'static,
        F: Fn(&A) -> B + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_map(&tb, f);
    }

    /// Wires the transmitter to the given receiver using a stateful fold function
    /// that returns an optional future. The future, if available, results in an
    /// `Option<B>`. In the case that the value of the future's result is `None`,
    /// no message will be sent to the given receiver.
    ///
    /// Lastly, a clean up function is ran at the completion of the future with its
    /// result.
    ///
    /// To aid in returning a viable future in your fold function, use
    /// `wrap_future`.
    pub fn wire_filter_fold_async<T, B, X, F, H>(&self, rb: &Receiver<B>, init: X, f: F, h: H)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + 'static,
        H: Fn(&mut T, &Option<B>) + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold_async(&tb, init, f, h);
    }
}


/// Receive messages instantly.
pub struct Receiver<A> {
    k: usize,
    responders: Rc<Responders<A>>,
}


/// Clone a receiver.
///
/// # Warning!
/// Be careful with this function. Because of magic, calling
/// [Receiver::respond] on a clone of a receiver sets the responder for both of
/// those receivers. **Under the hood they are the same responder**. This is why
/// [Receiver] has no [Clone] trait implementation.
///
/// Instead of cloning, if you need a new receiver that receives from the same
/// transmitter you should use [Receiver::branch], which comes in many flavors.
pub(crate) fn hand_clone<A>(rx: &Receiver<A>) -> Receiver<A> {
    Receiver {
        k: rx.k,
        responders: rx.responders.clone(),
    }
}


impl<A> Receiver<A> {
    pub fn new() -> Receiver<A> {
        Responders::recv_from(Default::default())
    }

    /// Set the response this receiver has to messages. Upon receiving a message
    /// the response will run immediately.
    pub fn respond<F>(self, f: F)
    where
        F: FnMut(&A) + 'static,
    {
        self.responders.insert(self.k, f);
    }

    /// Set the response this receiver has to messages. Upon receiving a message
    /// the response will run immediately.
    ///
    /// Folds mutably over a shared Rc<RefCell<T>>.
    pub fn respond_shared<T: 'static, F>(self, val: Rc<RefCell<T>>, f: F)
    where
        F: Fn(&mut T, &A) + 'static,
    {
        self.responders.insert(self.k, move |a: &A| {
            let mut t = val.borrow_mut();
            f(&mut t, a);
        });
    }

    /// Removes the responder from the receiver.
    /// This drops anything owned by the responder.
    pub fn drop_responder(&self) {
        self.responders.remove(self.k);
    }

    pub fn new_trns(&self) -> Transmitter<A> {
        Transmitter {
            responders: self.responders.clone(),
        }
    }

    /// Branch a receiver off of the original.
    /// Each branch will receive from the same transmitter.
    /// The new branch has no initial response to messages.
    pub fn branch(&self) -> Receiver<A> {
        self.responders.clone().recv_from()
    }

    /// Branch a new receiver off of an original and wire any messages sent to the
    /// original by using a stateful fold function.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the new receiver.
    ///
    /// Each branch will receive from the same transmitter.
    pub fn branch_filter_fold<B, X, T, F>(&self, init: X, f: F) -> Receiver<B>
    where
        B: 'static,
        X: Into<T>,
        T: 'static,
        F: Fn(&mut T, &A) -> Option<B> + 'static,
    {
        let ra = self.branch();
        let (tb, rb) = txrx();
        ra.forward_filter_fold(&tb, init, f);
        rb
    }

    /// Branch a new receiver off of an original and wire any messages sent to the
    /// original by using a stateful fold function, where the state is a shared
    /// mutex.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the new receiver.
    ///
    /// Each branch will receive from the same transmitter.
    pub fn branch_filter_fold_shared<B, T, F>(&self, state: Rc<RefCell<T>>, f: F) -> Receiver<B>
    where
        B: 'static,
        T: 'static,
        F: Fn(&mut T, &A) -> Option<B> + 'static,
    {
        let ra = self.branch();
        let (tb, rb) = txrx();
        ra.forward_filter_fold_shared(&tb, state, f);
        rb
    }

    /// Branch a new receiver off of an original and wire any messages sent to the
    /// original by using a stateful fold function.
    ///
    /// All output of the fold function is sent to the new receiver.
    ///
    /// Each branch will receive from the same transmitter(s).
    pub fn branch_fold<B, X, T, F>(&self, init: X, f: F) -> Receiver<B>
    where
        B: 'static,
        X: Into<T>,
        T: 'static,
        F: Fn(&mut T, &A) -> B + 'static,
    {
        let ra = self.branch();
        let (tb, rb) = txrx();
        ra.forward_fold(&tb, init, f);
        rb
    }

    /// Branch a new receiver off of an original and wire any messages sent to the
    /// original by using a stateful fold function, where the state is a shared
    /// mutex.
    ///
    /// All output of the fold function is sent to the new receiver.
    ///
    /// Each branch will receive from the same transmitter(s).
    pub fn branch_fold_shared<B, T, F>(&self, t: Rc<RefCell<T>>, f: F) -> Receiver<B>
    where
        B: 'static,
        T: 'static,
        F: Fn(&mut T, &A) -> B + 'static,
    {
        let ra = self.branch();
        let (tb, rb) = txrx();
        ra.forward_fold_shared(&tb, t, f);
        rb
    }

    /// Branch a new receiver off of an original and wire any messages sent to the
    /// original by using a stateless map function.
    ///
    /// The map function returns an `Option<B>`, representing an optional message
    /// to send to the new receiver.
    /// In the case that the result value of the map function is `None`, no message
    /// will be sent to the new receiver.
    ///
    /// Each branch will receive from the same transmitter.
    pub fn branch_filter_map<B, F>(&self, f: F) -> Receiver<B>
    where
        B: 'static,
        F: Fn(&A) -> Option<B> + 'static,
    {
        let ra = self.branch();
        let (tb, rb) = txrx();
        ra.forward_filter_map(&tb, f);
        rb
    }

    /// Branch a new receiver off of an original and wire any messages sent to the
    /// original by using a stateless map function.
    ///
    /// All output of the map function is sent to the new receiver.
    ///
    /// Each branch will receive from the same transmitter.
    pub fn branch_map<B, F>(&self, f: F) -> Receiver<B>
    where
        B: 'static,
        F: Fn(&A) -> B + 'static,
    {
        let ra = self.branch();
        let (tb, rb) = txrx();
        ra.forward_map(&tb, f);
        rb
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function, where the state is a shared mutex.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the transmitter.
    pub fn forward_filter_fold_shared<B, T, F>(self, tx: &Transmitter<B>, var: Rc<RefCell<T>>, f: F)
    where
        B: 'static,
        T: 'static,
        F: Fn(&mut T, &A) -> Option<B> + 'static,
    {
        let tx = tx.clone();
        self.respond(move |a: &A| {
            let result = {
                let mut t = var.borrow_mut();
                f(&mut t, a)
            };
            result.into_iter().for_each(|b| {
                tx.send(&b);
            });
        });
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the transmitter.
    pub fn forward_filter_fold<B, X, T, F>(self, tx: &Transmitter<B>, init: X, f: F)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<B> + 'static,
    {
        let var = Rc::new(RefCell::new(init.into()));
        self.forward_filter_fold_shared(tx, var, f);
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function. All output of the fold
    /// function is sent to the given transmitter.
    pub fn forward_fold<B, X, T, F>(self, tx: &Transmitter<B>, init: X, f: F)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> B + 'static,
    {
        self.forward_filter_fold(tx, init, move |t: &mut T, a: &A| Some(f(t, a)))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function, where the state is a shared mutex. All output of
    /// the fold function is sent to the given transmitter.
    pub fn forward_fold_shared<B, T, F>(self, tx: &Transmitter<B>, t: Rc<RefCell<T>>, f: F)
    where
        B: 'static,
        T: 'static,
        F: Fn(&mut T, &A) -> B + 'static,
    {
        self.forward_filter_fold_shared(tx, t, move |t: &mut T, a: &A| Some(f(t, a)))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateless map function. If the map function returns `None` for any messages
    /// those messages will *not* be sent to the given transmitter.
    pub fn forward_filter_map<B, F>(self, tx: &Transmitter<B>, f: F)
    where
        B: 'static,
        F: Fn(&A) -> Option<B> + 'static,
    {
        self.forward_filter_fold(tx, (), move |&mut (), a| f(a))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateless map function. All output of the map function is sent to the given
    /// transmitter.
    pub fn forward_map<B, F>(self, tx: &Transmitter<B>, f: F)
    where
        B: 'static,
        F: Fn(&A) -> B + 'static,
    {
        self.forward_filter_map(tx, move |a| Some(f(a)))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function that returns an optional future. The future, if
    /// returned, is executed. The future results in an `Option<B>`. In the case
    /// that the value of the future's result is `None`, no message will be sent to
    /// the transmitter.
    ///
    /// Lastly, a clean up function is ran at the completion of the future with its
    /// result.
    ///
    /// To aid in returning a viable future in your fold function, use
    /// `wrap_future`.
    // TODO: Examples of fold functions.
    pub fn forward_filter_fold_async<T, B, X, F, H>(self, tb: &Transmitter<B>, init: X, f: F, h: H)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + 'static,
        H: Fn(&mut T, &Option<B>) + 'static,
    {
        let state = Rc::new(RefCell::new(init.into()));
        let cleanup = Rc::new(Box::new(h));
        let tb = tb.clone();
        self.respond(move |a: &A| {
            let may_async = {
                let mut block_state = state.borrow_mut();
                f(&mut block_state, a)
            };
            may_async.into_iter().for_each(|block: RecvFuture<B>| {
                let tb_clone = tb.clone();
                let state_clone = state.clone();
                let cleanup_clone = cleanup.clone();
                let future = async move {
                    let opt: Option<B> = block.await;
                    opt.iter().for_each(|b| tb_clone.send(&b));
                    let mut inner_state = state_clone.borrow_mut();
                    cleanup_clone(&mut inner_state, &opt);
                };
                spawn_local(future);
            });
        });
    }

    /// Merge all the receivers into one. Any time a message is received on any
    /// receiver, it will be sent to the returned receiver.
    pub fn merge<B: 'static>(rxs: Vec<Receiver<B>>) -> Receiver<B> {
        let (tx, rx) = txrx();
        rxs.into_iter().for_each(|rx_inc| {
            let tx = tx.clone();
            rx_inc.branch().respond(move |a| {
                tx.send(a);
            });
        });
        rx
    }
}


/// Create a new unlinked `Receiver<T>`.
pub fn recv<A>() -> Receiver<A> {
    Receiver::new()
}


/// Create a new unlinked `Transmitter<T>`.
pub fn trns<A: 'static>() -> Transmitter<A> {
    Transmitter::new()
}


/// Create a linked `Transmitter<A>` and `Receiver<A>` pair.
pub fn txrx<A: 'static>() -> (Transmitter<A>, Receiver<A>) {
    let trns = Transmitter::new();
    let recv = trns.spawn_recv();
    (trns, recv)
}

/// Create a linked, filtering `Transmitter<A>` and `Receiver<B>` pair with
/// internal state.
///
/// Using the given filtering fold function, messages sent on the transmitter
/// will be folded into the given internal state and output messages may or may
/// not be sent to the receiver.
///
/// In the case that the return value of the given function is `None`, no message
/// will be sent to the receiver.
pub fn txrx_filter_fold<A, B, T, F>(t: T, f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: 'static,
    B: 'static,
    T: 'static,
    F: Fn(&mut T, &A) -> Option<B> + 'static,
{
    let (ta, ra) = txrx();
    let (tb, rb) = txrx();
    ra.forward_filter_fold(&tb, t, f);
    (ta, rb)
}

/// Create a linked, filtering `Transmitter<A>` and `Receiver<B>` pair with
/// shared state.
///
/// Using the given filtering fold function, messages sent on the transmitter
/// will be folded into the given shared state and output messages may or may
/// not be sent to the receiver.
///
/// In the case that the return value of the given function is `None`, no message
/// will be sent to the receiver.
pub fn txrx_filter_fold_shared<A, B, T, F>(
    var: Rc<RefCell<T>>,
    f: F,
) -> (Transmitter<A>, Receiver<B>)
where
    A: 'static,
    B: 'static,
    T: 'static,
    F: Fn(&mut T, &A) -> Option<B> + 'static,
{
    let (ta, ra) = txrx();
    let (tb, rb) = txrx();
    ra.forward_filter_fold_shared(&tb, var, f);
    (ta, rb)
}

/// Create a linked `Transmitter<A>` and `Receiver<B>` pair with internal state.
///
/// Using the given fold function, messages sent on the transmitter will be
/// folded into the given internal state and all output messages will be sent to
/// the receiver.
pub fn txrx_fold<A, B, T, F>(t: T, f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: 'static,
    B: 'static,
    T: 'static,
    F: Fn(&mut T, &A) -> B + 'static,
{
    let (ta, ra) = txrx();
    let (tb, rb) = txrx();
    ra.forward_fold(&tb, t, f);
    (ta, rb)
}

/// Create a linked `Transmitter<A>` and `Receiver<B>` pair with shared state.
///
/// Using the given fold function, messages sent on the transmitter are folded
/// into the given internal state and all output messages will be sent to the
/// receiver.
pub fn txrx_fold_shared<A, B, T, F>(t: Rc<RefCell<T>>, f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: 'static,
    B: 'static,
    T: 'static,
    F: Fn(&mut T, &A) -> B + 'static,
{
    let (ta, ra) = txrx();
    let (tb, rb) = txrx();
    ra.forward_fold_shared(&tb, t, f);
    (ta, rb)
}

/// Create a linked, filtering `Transmitter<A>` and `Receiver<B>` pair.
///
/// Using the given filtering map function, messages sent on the transmitter
/// are mapped to output messages that may or may not be sent to the receiver.
///
/// In the case that the return value of the given function is `None`, no message
/// will be sent to the receiver.
pub fn txrx_filter_map<A, B, F>(f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: 'static,
    B: 'static,
    F: Fn(&A) -> Option<B> + 'static,
{
    let (ta, ra) = txrx();
    let (tb, rb) = txrx();
    ra.forward_filter_map(&tb, f);
    (ta, rb)
}


/// Create a linked `Transmitter<A>` and `Receiver<B>` pair.
///
/// Using the given map function, messages sent on the transmitter are mapped
/// to output messages that will be sent to the receiver.
pub fn txrx_map<A, B, F>(f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: 'static,
    B: 'static,
    F: Fn(&A) -> B + 'static,
{
    let (ta, ra) = txrx();
    let (tb, rb) = txrx();
    ra.forward_map(&tb, f);
    (ta, rb)
}


/// Helper for making thread-safe shared mutable variables.
///
/// Use this as a short hand for creating variables to pass to
/// the many `*_shared` flavored fold functions in the [txrx](index.html)
/// module.
pub fn new_shared<A: 'static, X: Into<A>>(init: X) -> Rc<RefCell<A>> {
    Rc::new(RefCell::new(init.into()))
}


#[cfg(test)]
mod range {
    #[test]
    fn range() {
        let mut n = 0;
        for i in 0..3 {
            n = i;
        }

        assert_eq!(n, 2);
    }
}


#[cfg(test)]
mod instant_txrx {
    use super::*;

    #[test]
    fn txrx_test() {
        let count = Rc::new(RefCell::new(0));
        let (tx_unit, rx_unit) = txrx::<()>();
        let (tx_i32, rx_i32) = txrx::<i32>();
        {
            let my_count = count.clone();
            rx_i32.respond(move |n: &i32| {
                println!("Got message: {:?}", n);
                let mut c = my_count.borrow_mut();
                *c = *n;
            });

            let mut n = 0;
            rx_unit.respond(move |()| {
                n += 1;
                tx_i32.send(&n);
            })
        }

        tx_unit.send(&());
        tx_unit.send(&());
        tx_unit.send(&());

        let final_count: i32 = *count.borrow();
        assert_eq!(final_count, 3);
    }

    #[test]
    fn wire_txrx() {
        let tx_unit = Transmitter::<()>::new();
        let rx_str = Receiver::<String>::new();
        tx_unit.wire_filter_fold(&rx_str, 0, |n: &mut i32, &()| -> Option<String> {
            *n += 1;
            if *n > 2 {
                Some(format!("Passed 3 incoming messages ({})", *n))
            } else {
                None
            }
        });

        let got_called = Rc::new(RefCell::new(false));
        let remote_got_called = got_called.clone();
        rx_str.respond(move |s: &String| {
            println!("got: {:?}", s);
            let mut called = remote_got_called.borrow_mut();
            *called = true;
        });

        tx_unit.send(&());
        tx_unit.send(&());
        tx_unit.send(&());

        let ever_called = *got_called.borrow();
        assert!(ever_called);
    }

    #[test]
    fn branch_map() {
        let (tx, rx) = txrx::<()>();
        let ry: Receiver<i32> = rx.branch_map(|_| 0);

        let done = Rc::new(RefCell::new(false));

        let cdone = done.clone();
        ry.respond(move |n| {
            if *n == 0 {
                *cdone.borrow_mut() = true;
            }
        });

        tx.send(&());

        assert!(*done.borrow());
    }
}

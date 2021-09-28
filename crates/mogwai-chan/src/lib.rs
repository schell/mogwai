//! # Multiple producer, multiple consumer channels that respond to messages
//! instantly. Just add water! ;)
//!
//! ## Creating channels
//! There are a number of ways to create a channel in this module. The most
//! straight forward is to use the function [channel]. This will create a linked
//! [Transmitter] + [Receiver] pair:
//!
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let (tx, rx): (Transmitter<()>, Receiver<()>) = channel();
//! ```
//!
//! Or maybe you prefer an alternative syntax:
//!
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let (tx, rx) = channel::<()>();
//! ```
//!
//! This pair makes a linked channel. Messages you send on the [Transmitter]
//! will be sent directly to the [Receiver] on the other end.
//!
//! You can create separate terminals using the [`Transmitter::default`] and
//! [`Receiver::default`] functions. Then later in your code you can spawn new
//! linked partners from them:
//!
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let mut tx: Transmitter<()> = Transmitter::default();
//! let rx = tx.spawn_recv();
//! tx.send(&()); // rx will receive the message
//! ```
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let rx: Receiver<()> = Receiver::default();
//! let tx = rx.new_trns();
//! tx.send(&()); // rx will receive the message
//! ```
//!
//! ## Sending messages
//!
//! Once you have a channel pair you can start sending messages:
//!
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let (tx, rx) = channel();
//! tx.send(&());
//! tx.send(&());
//! tx.send(&());
//! ```
//!
//! Notice that we send references. This is because neither the transmitter nor
//! the receiver own the messages.
//!
//! It's also possible to send asynchronous messages! We can do this with
//! [Transmitter::send_async], which takes a type that implements [Future].
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
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let (tx, rx) = channel();
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
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let shared_count = new_shared(0);
//! let (tx, rx) = channel();
//! rx.respond_shared(shared_count.clone(), |count: &mut i32, &()| {
//!     *count += 1;
//!     println!("{} messages received!", *count);
//! });
//! tx.send(&());
//! tx.send(&());
//! tx.send(&());
//! assert_eq!(shared_count.visit(|v| *v), 3);
//! ```
//!
//! ## Composing channels
//!
//! Sending messages into a transmitter and having it pop out automatically is
//! great, but wait, there's more! What if we have a `tx_a:Transmitter<A>` and a
//! `rx_b:Receiver<B>`, but we want to send `A`s on `tx_a` and have `B`s pop out
//! of `rx_b`? We could use the machinery we have and write something like:
//!
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let (tx_a, rx_b) = {
//!   let (tx_a, rx_a) = channel::<u32>();
//!   let (tx_b, rx_b) = channel::<String>();
//!   let f = |a: &u32| { format!("{}", a) };
//!   rx_a.respond(move |a| {
//!     tx_b.send(&f(a));
//!   });
//!   (tx_a, rx_b)
//! };
//! ```
//!
//! And indeed, it works! But that's an awful lot of boilerplate just to get a
//! channel of `A`s to `B`s. Instead we can use the `channel_map` function, which
//! does all of this for us given the map function. Here's an example using
//! a `Transmitter<()>` that sends to a `Receiver<i32>`:
//!
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! // For every unit that gets sent, map it to `1:i32`.
//! let (tx_a, rx_b) = channel_map(|&()| 1);
//! let shared_count = new_shared(0);
//! rx_b.respond_shared(shared_count.clone(), |count: &mut i32, n: &i32| {
//!     *count += n;
//!     println!("Current count is {}", *count);
//! });
//!
//! tx_a.send(&());
//! tx_a.send(&());
//! tx_a.send(&());
//! assert_eq!(shared_count.visit(|v| *v), 3);
//! ```
//!
//! That is useful, but we can also do much more than simple maps! We can fold
//! over an internal state or a shared state, we can filter some of the sent
//! messages and we can do all those things together! Check out the `channel_*`
//! family of functions:
//!
//! * [channel]
//! * [channel_filter_fold]
//! * [channel_filter_fold_shared]
//! * [channel_filter_map]
//! * [channel_fold]
//! * [channel_fold_shared]
//! * [channel_map]
//!
//! You'll also find functions with these flavors in [Transmitter] and
//! [Receiver].
//!
//! ## Wiring [Transmitter]s and forwading [Receiver]s
//!
//! Another way to get a channel pair of different types is to create each side
//! separately using [trns] and [recv] and then wire them together:
//!
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let mut tx: Transmitter<()> = Transmitter::new();
//! let rx: Receiver<i32> = Receiver::new();
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
//! Conversely, if you would like to forward messages from a receiver into a
//! transmitter of a different type you can "forward" messages from the receiver
//! to the transmitter:
//!
//! ```rust
//! # extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let (tx, rx) = channel::<()>();
//! let (mut tx_i32, rx_i32) = channel::<i32>();
//! rx.forward_map(&tx_i32, |&()| 1);
//!
//! let shared_got_it = new_shared(false);
//! rx_i32.respond_shared(shared_got_it.clone(), |got_it: &mut bool, n: &i32| {
//!     println!("Got {}", *n);
//!     *got_it = true;
//! });
//!
//! tx.send(&());
//! assert_eq!(shared_got_it.visit(|v| *v), true);
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
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let (tx1, rx) = channel();
//! let tx2 = tx1.clone();
//! let shared_count = new_shared(0);
//! rx.respond_shared(shared_count.clone(), |count: &mut i32, &()| {
//!     *count += 1;
//! });
//! tx1.send(&());
//! tx2.send(&());
//! assert_eq!(shared_count.visit(|v| *v), 2);
//! ```
//!
//! [Receiver]s are a bit different from [Transmitter]s, though. They are _not_
//! clonable because they house a responder, which must be unique. Instead we can
//! use [Receiver::branch] to create a new receiver that is linked to the same
//! transmitters as the original, but owns its own unique response to messages:
//!
//! ```rust
//! extern crate mogwai_chan;
//! use mogwai_chan::*;
//!
//! let (tx, rx1) = channel();
//! let rx2 = rx1.branch();
//! let shared_count = new_shared(0);
//! rx1.respond_shared(shared_count.clone(), |count: &mut i32, &()| {
//!     *count += 1;
//! });
//! rx2.respond_shared(shared_count.clone(), |count: &mut i32, &()| {
//!     *count += 1;
//! });
//! tx.send(&());
//! assert_eq!(shared_count.visit(|v| *v), 2);
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

mod channel;
pub mod effect;
pub mod model;
pub mod patch;

pub use channel::*;

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
mod test {
    use super::{model::*, patch::*, *};
    use std::sync::{Arc, Mutex};

    #[test]
    fn channel_test() {
        let count = new_shared(0);
        let (tx_unit, rx_unit) = channel::<()>();
        let (tx_i32, rx_i32) = channel::<i32>();
        {
            let my_count = count.clone();
            rx_i32.respond(move |n: &i32| {
                println!("Got message: {:?}", n);
                my_count.visit_mut(|c| *c = *n);
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

        let final_count: i32 = count.visit(|c| *c);
        assert_eq!(final_count, 3);
    }

    #[test]
    fn wire_channel() {
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

        let got_called = Arc::new(Mutex::new(false));
        let remote_got_called = got_called.clone();
        rx_str.respond(move |s: &String| {
            println!("got: {:?}", s);
            let mut called = remote_got_called.lock().unwrap();
            *called = true;
        });

        tx_unit.send(&());
        tx_unit.send(&());
        tx_unit.send(&());

        let ever_called = *got_called.lock().unwrap();
        assert!(ever_called);
    }

    #[test]
    fn branch_map() {
        let (tx, rx) = channel::<()>();
        let ry: Receiver<i32> = rx.branch_map(|_| 0);
        let done = new_shared(false);
        ry.respond_shared(done.clone(), move |d, n| {
            if *n == 0 {
                *d = true;
            }
        });

        tx.send(&());
        assert!(done.visit(|d| *d));
    }

    #[test]
    fn patch_list_model() {
        println!("patch_list_model");
        let mut list = PatchListModel::new(vec![]);
        let view = new_shared(vec![]);
        list.receiver()
            .branch()
            .respond_shared(
                view.clone(),
                |v: &mut Vec<f32>, patch: &Patch<i32>| {
                    println!("got patch: {:?}", patch);
                    let patch = patch.patch_map(|i| *i as f32);
                    v.patch_apply(patch);
                }
            );

        list.patch_push(0);
        list.patch_push(1);
        list.patch_push(2);
        list.patch_push(3);
        list.patch_push(4);
        assert_eq!(view.visit(Vec::clone), vec![0.0, 1.0, 2.0, 3.0, 4.0]);

        let _ = list.patch_splice(0.., vec![]);
        assert!(view.visit(Vec::is_empty));
    }
}

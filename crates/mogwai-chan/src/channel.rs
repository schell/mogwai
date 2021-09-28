//! Instant [`Transmitters`] and [`Receivers`].
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

mod target;
pub use target::*;

/// Wrap an optional future message in a pin box.
pub fn wrap_future<A, F>(future: F) -> Option<RecvFuture<A>>
where
    F: Future<Output = Option<A>> + Send + 'static,
{
    Some(Box::pin(future))
}

/// The sending end of an instant channel.
pub struct Transmitter<A> {
    responders: Counted<Responders<A>>,
}

impl<A> Clone for Transmitter<A> {
    fn clone(&self) -> Self {
        Self {
            responders: self.responders.clone(),
        }
    }
}

impl<A> Default for Transmitter<A> {
    fn default() -> Self {
        Transmitter {
            responders: Default::default(),
        }
    }
}

impl<A: Transmission> Transmitter<A> {
    pub fn new() -> Transmitter<A> {
        Default::default()
    }

    /// Spawn a receiver for this transmitter.
    pub fn spawn_recv(&self) -> Receiver<A> {
        Receiver::from(self.responders.clone())
    }

    /// Send a message to any and all receivers of this transmitter.
    ///
    /// The responder closures of any downstream [`Receiver`]s are executed immediately.
    pub fn send(&self, a: &A) {
        self.responders.send(a);
    }

    /// Send a bunch of messages.
    ///
    /// The responder closures of any downstream [`Receiver`]s are executed immediately.
    pub fn send_many(&self, msgs: &[A]) {
        msgs.iter().for_each(|msg| self.send(msg));
    }

    /// Execute a future that results in a message, then send it. `wasm32` spawns
    /// a local execution context to drive the `Future` to completion. Outside of
    /// `wasm32` (e.g. during server-side rendering) this is a noop.
    ///
    /// ### Notes
    ///
    /// Does not exist outside of the wasm32 architecture because the
    /// functionality of [`wasm_bindgen_futures::spawn_local`] is largely
    /// managed by third party runtimes that mogwai does not need to depend
    /// upon. If `send_async` is necessary for server side rendering it may be
    /// better to modify the behavior so the [`Future`] resolves outside of the
    /// `Transmitter` lifecycle.
    ///
    /// ```rust, ignore
    /// extern crate mogwai;
    /// extern crate web_sys;
    /// use mogwai::prelude::*;
    /// use web_sys::{Request, RequestMode, RequestInit, Response};
    ///
    /// // Here's our async function that fetches a text response from a server,
    /// // or returns an error string.
    /// async fn request_to_text(req:Request) -> Result<String, String> {
    ///   let resp:Response =
    ///     JsFuture::from(
    ///       window()
    ///         .fetch_with_request(&req)
    ///     )
    ///     .await
    ///     .map_err(|_| "request failed".to_string())?
    ///     .dyn_into()
    ///     .map_err(|_| "response is malformed")?;
    ///   let text:String =
    ///     JsFuture::from(
    ///       resp
    ///         .text()
    ///         .map_err(|_| "could not get response text")?
    ///     )
    ///     .await
    ///     .map_err(|_| "getting text failed")?
    ///     .as_string()
    ///     .ok_or("couldn't get text as string".to_string())?;
    ///   Ok(text)
    /// }
    ///
    /// let (tx, rx) = txrx();
    /// tx.send_async(async {
    ///   let mut opts = RequestInit::new();
    ///   opts.method("GET");
    ///   opts.mode(RequestMode::Cors);
    ///
    ///   let req =
    ///     Request::new_with_str_and_init(
    ///       "https://worldtimeapi.org/api/timezone/Europe/London.txt",
    ///       &opts
    ///     )
    ///     .unwrap_throw();
    ///
    ///   request_to_text(req)
    ///     .await
    ///     .unwrap_or_else(|e| e)
    /// });
    /// ```
    #[cfg(target_arch = "wasm32")]
    pub fn send_async<FutureA: FutureMessage<A>>(&self, fa: FutureA) {
        let tx = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let a: A = fa.await;
            tx.send(&a);
        });
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "async-tokio"))]
    pub fn send_async<FutureA: FutureMessage<A>>(&self, fa: FutureA) {
        let _ = tokio::task::spawn(fa);
    }
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "async-tokio"), feature = "async-smol"))]
    pub fn send_async<FutureA: FutureMessage<A>>(&self, fa: FutureA) {
        let _ = smol::spawn(fa);
    }
    /// Extend this transmitter with a new transmitter using a filtering fold
    /// function. The given function folds messages of `B` over a shared state `T`
    /// and optionally sends `A`s down into this transmitter.
    pub fn contra_filter_fold_shared<B, T, F>(&self, var: Counted<Shared<T>>, f: F) -> Transmitter<B>
    where
        B: Transmission,
        T: Transmission,
        F: Fn(&mut T, &B) -> Option<A> + Transmission,
    {
        let tx = self.clone();
        let (tev, rev) = channel();
        rev.respond(move |ev| {
            let result = var.visit_mut(|t| f(t, ev));
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
        B: Transmission,
        T: Transmission,
        X: Into<T>,
        F: Fn(&mut T, &B) -> Option<A> + Transmission,
    {
        let tx = self.clone();
        let (tev, rev) = channel();
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
        B: Transmission,
        T: Transmission,
        X: Into<T>,
        F: Fn(&mut T, &B) -> A + Transmission,
    {
        self.contra_filter_fold(init, move |t, ev| Some(f(t, ev)))
    }

    /// Extend this transmitter with a new transmitter using a filter function.
    /// The given function maps messages of `B` and optionally sends `A`s into this
    /// transmitter.
    pub fn contra_filter_map<B, F>(&self, f: F) -> Transmitter<B>
    where
        B: Transmission,
        F: Fn(&B) -> Option<A> + Transmission,
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
        B: Transmission,
        F: Fn(&B) -> A + Transmission,
    {
        self.contra_filter_map(move |ev| Some(f(ev)))
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function, where the state is a shared mutex.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the receiver.
    pub fn wire_filter_fold_shared<T, B, F>(&self, rb: &Receiver<B>, var: Counted<Shared<T>>, f: F)
    where
        B: Transmission,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> Option<B> + Transmission,
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
        B: Transmission,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<B> + Transmission,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold(&tb, init, f);
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function.
    pub fn wire_fold<T, B, X, F>(&self, rb: &Receiver<B>, init: X, f: F)
    where
        B: Transmission,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> B + Transmission,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_fold(&tb, init, f);
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function, where the state is a shared mutex.
    pub fn wire_fold_shared<T, B, F>(&self, rb: &Receiver<B>, var: Counted<Shared<T>>, f: F)
    where
        B: Transmission,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> B + Transmission,
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
        B: Transmission,
        F: Fn(&A) -> Option<B> + Transmission,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_map(&tb, f);
    }

    /// Wires the transmitter to the given receiver using a stateless map function.
    pub fn wire_map<B, F>(&self, rb: &Receiver<B>, f: F)
    where
        B: Transmission,
        F: Fn(&A) -> B + Transmission,
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
        B: Transmission,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + Transmission,
        H: Fn(&mut T, &Option<B>) + Transmission,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold_async(&tb, init, f, h);
    }
}

// A message received by a [`Receiver`] at some point in the future.
struct MessageFuture<A> {
    var: Counted<Shared<Option<A>>>,
    waker: Counted<Shared<Option<Waker>>>,
}

impl<A: Clone + Transmission> From<Receiver<A>> for MessageFuture<A> {
    fn from(rx: Receiver<A>) -> Self {
        let var: Counted<Shared<Option<A>>> = Default::default();
        let var2: Counted<Shared<Option<A>>> = var.clone();
        let waker: Counted<Shared<Option<Waker>>> = Default::default();
        let waker2 = waker.clone();
        rx.respond(move |msg| {
            var2.visit_mut(|v| *v = Some(msg.clone()));
            waker2.visit_mut(|w| w.take().into_iter().for_each(|w| w.wake()));
        });

        MessageFuture { var, waker }
    }
}

impl<A: Transmission> Future for MessageFuture<A> {
    type Output = A;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let var: Option<A> = self.var.visit_mut(Option::take);
        match var {
            Some(msg) => Poll::Ready(msg),
            None => {
                self.waker.visit_mut(|w| *w = Some(ctx.waker().clone()) );
                Poll::Pending
            }
        }
    }
}

impl<A: Transmission> futures::Stream for MessageFuture<A> {
    type Item = A;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        match <Self as Future>::poll(self, cx) {
            Poll::Ready(msg) => Poll::Ready(Some(msg)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Receive messages instantly.
pub struct Receiver<A> {
    k: usize,
    responders: Counted<Responders<A>>,
}

impl<A: Transmission> From<Counted<Responders<A>>> for Receiver<A> {
    fn from(responders: Counted<Responders<A>>) -> Receiver<A> {
        let k = responders.get_next_k();
        Receiver { k, responders }
    }
}

impl<A: Transmission> Clone for Receiver<A> {
    fn clone(&self) -> Self {
        Receiver::from(self.responders.clone())
    }
}

impl<A: Transmission> Default for Receiver<A> {
    fn default() -> Self {
        Receiver::from(Counted::new(Responders::default()))
    }
}

impl<A: Transmission> Receiver<A> {
    /// Create a new Receiver.
    pub fn new() -> Receiver<A> {
        Default::default()
    }

    /// Set the response this receiver has to messages. Upon receiving a message
    /// the response will run immediately.
    pub fn respond<F>(self, f: F)
    where
        F: FnMut(&A) + Transmission,
    {
        self.responders.insert(self.k, f);
    }

    /// Set the response this receiver has to messages. Upon receiving a message
    /// the response will run immediately.
    ///
    /// Folds mutably over a Counted<Shared<T>>.
    pub fn respond_shared<T, F>(self, val: Counted<Shared<T>>, f: F)
    where
        T: 'static + Send,
        F: Fn(&mut T, &A) + Transmission,
    {
        self.responders.insert(self.k, move |a: &A| {
            val.visit_mut(|t| f(t, a));
        });
    }

    /// Removes the responder from the receiver.
    /// This drops anything owned by the responder.
    pub fn drop_responder(&self) {
        self.responders.remove(self.k);
    }

    /// Spawn a new [`Transmitter`] that sends to this Receiver.
    pub fn new_trns(&self) -> Transmitter<A> {
        Transmitter {
            responders: self.responders.clone(),
        }
    }

    /// Branch a receiver off of the original.
    /// Each branch will receive from the same transmitter.
    /// The new branch has no initial response to messages.
    pub fn branch(&self) -> Receiver<A> {
        Receiver::from(self.responders.clone())
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
        B: Transmission,
        X: Into<T>,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> Option<B> + Transmission,
    {
        let ra = self.branch();
        let (tb, rb) = channel();
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
    pub fn branch_filter_fold_shared<B, T, F>(&self, state: Counted<Shared<T>>, f: F) -> Receiver<B>
    where
        B: Transmission,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> Option<B> + Transmission,
    {
        let ra = self.branch();
        let (tb, rb) = channel();
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
        B: Transmission,
        X: Into<T>,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> B + Transmission,
    {
        let ra = self.branch();
        let (tb, rb) = channel();
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
    pub fn branch_fold_shared<B, T, F>(&self, t: Counted<Shared<T>>, f: F) -> Receiver<B>
    where
        B: Transmission,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> B + Transmission,
    {
        let ra = self.branch();
        let (tb, rb) = channel();
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
        B: Transmission,
        F: Fn(&A) -> Option<B> + Transmission,
    {
        let ra = self.branch();
        let (tb, rb) = channel();
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
        B: Transmission,
        F: Fn(&A) -> B + Transmission,
    {
        let ra = self.branch();
        let (tb, rb) = channel();
        ra.forward_map(&tb, f);
        rb
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function, where the state is a shared mutex.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the transmitter.
    pub fn forward_filter_fold_shared<B, T, F>(self, tx: &Transmitter<B>, var: Counted<Shared<T>>, f: F)
    where
        B: Transmission,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> Option<B> + Transmission,
    {
        let tx = tx.clone();
        self.respond(move |a: &A| {
            if let Some(b) = var.visit_mut(|t| f(t, a)) {
                tx.send(&b);
            }
        });
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the transmitter.
    pub fn forward_filter_fold<B, X, T, F>(self, tx: &Transmitter<B>, init: X, f: F)
    where
        B: Transmission,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<B> + Transmission,
    {
        let var = Counted::new(Shared::new(init.into()));
        self.forward_filter_fold_shared(tx, var, f);
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function. All output of the fold
    /// function is sent to the given transmitter.
    pub fn forward_fold<B, X, T, F>(self, tx: &Transmitter<B>, init: X, f: F)
    where
        B: Transmission,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> B + Transmission,
    {
        self.forward_filter_fold(tx, init, move |t: &mut T, a: &A| Some(f(t, a)))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function, where the state is a shared mutex. All output of
    /// the fold function is sent to the given transmitter.
    pub fn forward_fold_shared<B, T, F>(self, tx: &Transmitter<B>, t: Counted<Shared<T>>, f: F)
    where
        B: Transmission,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> B + Transmission,
    {
        self.forward_filter_fold_shared(tx, t, move |t: &mut T, a: &A| Some(f(t, a)))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateless map function. If the map function returns `None` for any messages
    /// those messages will *not* be sent to the given transmitter.
    pub fn forward_filter_map<B, F>(self, tx: &Transmitter<B>, f: F)
    where
        B: Transmission,
        F: Fn(&A) -> Option<B> + Transmission,
    {
        self.forward_filter_fold(tx, (), move |&mut (), a| f(a))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateless map function. All output of the map function is sent to the given
    /// transmitter.
    pub fn forward_map<B, F>(self, tx: &Transmitter<B>, f: F)
    where
        B: Transmission,
        F: Fn(&A) -> B + Transmission,
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
    pub fn forward_filter_fold_async<T, B, X, F, H>(self, tb: &Transmitter<B>, init: X, f: F, h: H)
    where
        B: Transmission,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + Transmission,
        H: Fn(&mut T, &Option<B>) + Transmission,
    {
        let state = new_shared(init.into());
        let cleanup = Counted::new(Box::new(h));
        let tb = tb.clone();
        self.respond(move |a: &A| {
            if let Some(block) = state.visit_mut(|s| f(s, a)) {
                let tb_clone = tb.clone();
                let state_clone = state.clone();
                let cleanup_clone = cleanup.clone();
                let future = async move {
                    let opt: Option<B> = block.await;
                    opt.iter().for_each(|b| tb_clone.send(&b));
                    state_clone.visit_mut(|t| cleanup_clone(t, &opt));
                };
                target::spawn(future);
            }
        });
    }

    /// Merge all the receivers into one. Any time a message is received on any
    /// receiver, it will be sent to the returned receiver.
    pub fn merge<B>(rxs: Vec<Receiver<B>>) -> Receiver<B>
    where
        B: Transmission,
    {
        let (tx, rx) = channel();
        rxs.into_iter().for_each(|rx_inc| {
            let tx = tx.clone();
            rx_inc.branch().respond(move |a| {
                tx.send(a);
            });
        });
        rx
    }

    /// Create a future to await the next message received by this `Receiver`.
    pub fn recv(&self) -> impl Future<Output = A>
    where
        A: Clone
    {
        MessageFuture::from(self.branch())
    }

    /// Create a future to await the next message received by this `Receiver`.
    pub fn recv_stream(&self) -> impl futures::Stream<Item = A>
    where
        A: Clone
    {
        MessageFuture::from(self.branch())
    }

}

/// Create a linked `Transmitter<A>` and `Receiver<A>` pair.
pub fn channel<A>() -> (Transmitter<A>, Receiver<A>)
where
    A: Transmission,
{
    let trns: Transmitter<A> = Default::default();
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
pub fn channel_filter_fold<A, B, T, F>(t: T, f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: Transmission,
    B: Transmission,
    T: Send + 'static,
    F: Fn(&mut T, &A) -> Option<B> + Transmission,
{
    let (ta, ra) = channel();
    let (tb, rb) = channel();
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
pub fn channel_filter_fold_shared<A, B, T, F>(
    var: Counted<Shared<T>>,
    f: F,
) -> (Transmitter<A>, Receiver<B>)
where
    A: Transmission,
    B: Transmission,
    T: Send + 'static,
    F: Fn(&mut T, &A) -> Option<B> + Transmission,
{
    let (ta, ra) = channel();
    let (tb, rb) = channel();
    ra.forward_filter_fold_shared(&tb, var, f);
    (ta, rb)
}

/// Create a linked `Transmitter<A>` and `Receiver<B>` pair with internal state.
///
/// Using the given fold function, messages sent on the transmitter will be
/// folded into the given internal state and all output messages will be sent to
/// the receiver.
pub fn channel_fold<A, B, T, F>(t: T, f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: Transmission,
    B: Transmission,
    T: Send + 'static,
    F: Fn(&mut T, &A) -> B + Transmission,
{
    let (ta, ra) = channel();
    let (tb, rb) = channel();
    ra.forward_fold(&tb, t, f);
    (ta, rb)
}

/// Create a linked `Transmitter<A>` and `Receiver<B>` pair with shared state.
///
/// Using the given fold function, messages sent on the transmitter are folded
/// into the given internal state and all output messages will be sent to the
/// receiver.
pub fn channel_fold_shared<A, B, T, F>(t: Counted<Shared<T>>, f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: Transmission,
    B: Transmission,
    T: Send + 'static,
    F: Fn(&mut T, &A) -> B + Transmission,
{
    let (ta, ra) = channel();
    let (tb, rb) = channel();
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
pub fn channel_filter_map<A, B, F>(f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: Transmission,
    B: Transmission,
    F: Fn(&A) -> Option<B> + Transmission,
{
    let (ta, ra) = channel();
    let (tb, rb) = channel();
    ra.forward_filter_map(&tb, f);
    (ta, rb)
}

/// Create a linked `Transmitter<A>` and `Receiver<B>` pair.
///
/// Using the given map function, messages sent on the transmitter are mapped
/// to output messages that will be sent to the receiver.
pub fn channel_map<A, B, F>(f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: Transmission,
    B: Transmission,
    F: Fn(&A) -> B + Transmission,
{
    let (ta, ra) = channel();
    let (tb, rb) = channel();
    ra.forward_map(&tb, f);
    (ta, rb)
}

/// Helper for making thread-safe shared mutable variables.
///
/// Use this as a short hand for creating variables to pass to
/// the many `*_shared` flavored fold functions in the [channel](index.html)
/// module.
pub fn new_shared<A: 'static, X: Into<A>>(init: X) -> Counted<Shared<A>> {
    Counted::new(Shared::new(init.into()))
}

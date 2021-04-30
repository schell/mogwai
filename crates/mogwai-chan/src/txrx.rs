use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll, Waker},
};

/// A pinned, possible future message.
#[cfg(target_arch = "wasm32")]
pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<A>>>>;
#[cfg(not(target_arch = "wasm32"))]
pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

/// Wrap an optional future message in a pin box.
pub fn wrap_future<A, F>(future: F) -> Option<RecvFuture<A>>
where
    F: Future<Output = Option<A>> + Send + 'static,
{
    Some(Box::pin(future))
}

struct Responders<A> {
    next_k: AtomicUsize,
    branches: Mutex<HashMap<usize, Box<dyn FnMut(&A) + Send + Sync>>>,
}

impl<A> Default for Responders<A> {
    fn default() -> Self {
        Self {
            next_k: AtomicUsize::new(0),
            branches: Default::default(),
        }
    }
}

impl<A> Responders<A> {
    fn insert(&self, k: usize, f: impl FnMut(&A) + Send + Sync + 'static) {
        let mut guard = self.branches.lock().unwrap();
        guard.insert(k, Box::new(f));
    }

    fn remove(&self, k: usize) {
        let mut guard = self.branches.lock().unwrap();
        guard.remove(&k);
    }

    fn send(&self, a: &A) {
        let mut guard = self.branches.lock().unwrap();
        guard.values_mut().for_each(|f| {
            f(a);
        });
    }
}

/// Send messages instantly.
pub struct Transmitter<A> {
    responders: Arc<Responders<A>>,
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

impl<A: Send + Sync + 'static> Transmitter<A> {
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
    pub fn send_async<FutureA>(&self, fa: FutureA)
    where
        FutureA: Future<Output = A> + Send + 'static,
    {
        let tx = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let a: A = fa.await;
            tx.send(&a);
        });
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "async-tokio"))]
    pub fn send_async<FutureA>(&self, fa: FutureA)
    where
        FutureA: Future<Output = A> + Send + 'static,
    {
        let _ = tokio::task::spawn(fa);
    }
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "async-tokio")))]
    pub fn send_async<FutureA>(&self, fa: FutureA)
    where
        FutureA: Future<Output = A> + Send + 'static,
    {
        compile_error!("Transmitter::send_async is un implemented. Either compile for wasm32 or choose an async implementation using cargo features")
    }

    /// Extend this transmitter with a new transmitter using a filtering fold
    /// function. The given function folds messages of `B` over a shared state `T`
    /// and optionally sends `A`s down into this transmitter.
    pub fn contra_filter_fold_shared<B, T, F>(&self, var: Arc<Mutex<T>>, f: F) -> Transmitter<B>
    where
        B: Send + Sync + 'static,
        T: Send + Sync + 'static,
        F: Fn(&mut T, &B) -> Option<A> + Send + Sync + 'static,
    {
        let tx = self.clone();
        let (tev, rev) = channel();
        rev.respond(move |ev| {
            let result = {
                let mut guard = var.lock().unwrap();
                f(&mut guard, ev)
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
        B: Send + Sync + 'static,
        T: Send + Sync + 'static,
        X: Into<T>,
        F: Fn(&mut T, &B) -> Option<A> + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        T: Send + Sync + 'static,
        X: Into<T>,
        F: Fn(&mut T, &B) -> A + Send + Sync + 'static,
    {
        self.contra_filter_fold(init, move |t, ev| Some(f(t, ev)))
    }

    /// Extend this transmitter with a new transmitter using a filter function.
    /// The given function maps messages of `B` and optionally sends `A`s into this
    /// transmitter.
    pub fn contra_filter_map<B, F>(&self, f: F) -> Transmitter<B>
    where
        B: Send + Sync + 'static,
        F: Fn(&B) -> Option<A> + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        F: Fn(&B) -> A + Send + Sync + 'static,
    {
        self.contra_filter_map(move |ev| Some(f(ev)))
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function, where the state is a shared mutex.
    ///
    /// The fold function returns an `Option<B>`. In the case that the value of
    /// `Option<B>` is `None`, no message will be sent to the receiver.
    pub fn wire_filter_fold_shared<T, B, F>(&self, rb: &Receiver<B>, var: Arc<Mutex<T>>, f: F)
    where
        B: Send + Sync + 'static,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> Option<B> + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<B> + Send + Sync + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold(&tb, init, f);
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function.
    pub fn wire_fold<T, B, X, F>(&self, rb: &Receiver<B>, init: X, f: F)
    where
        B: Send + Sync + 'static,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> B + Send + Sync + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_fold(&tb, init, f);
    }

    /// Wires the transmitter to send to the given receiver using a stateful fold
    /// function, where the state is a shared mutex.
    pub fn wire_fold_shared<T, B, F>(&self, rb: &Receiver<B>, var: Arc<Mutex<T>>, f: F)
    where
        B: Send + Sync + 'static,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> B + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        F: Fn(&A) -> Option<B> + Send + Sync + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_map(&tb, f);
    }

    /// Wires the transmitter to the given receiver using a stateless map function.
    pub fn wire_map<B, F>(&self, rb: &Receiver<B>, f: F)
    where
        B: Send + Sync + 'static,
        F: Fn(&A) -> B + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + Send + Sync + 'static,
        H: Fn(&mut T, &Option<B>) + Send + Sync + 'static,
    {
        let tb = rb.new_trns();
        let ra = self.spawn_recv();
        ra.forward_filter_fold_async(&tb, init, f, h);
    }
}

// A message received by a [`Receiver`] at some point in the future.
struct MessageFuture<A> {
    var: Arc<Mutex<Option<A>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<A> Future for MessageFuture<A> {
    type Output = A;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let future: &mut MessageFuture<A> = self.get_mut();
        let var: Option<A> = {
            let mut guard = future.var.lock().unwrap();
            guard.take()
        };
        match var {
            Some(msg) => Poll::Ready(msg),
            None => {
                let mut guard = future.waker.lock().unwrap();
                *guard = Some(ctx.waker().clone());
                Poll::Pending
            }
        }
    }
}

/// Receive messages instantly.
pub struct Receiver<A> {
    k: usize,
    responders: Arc<Responders<A>>,
}

impl<A> From<Arc<Responders<A>>> for Receiver<A> {
    fn from(responders: Arc<Responders<A>>) -> Receiver<A> {
        let k = responders.next_k.fetch_add(1, Ordering::SeqCst);

        Receiver { k, responders }
    }
}

impl<A> Clone for Receiver<A> {
    fn clone(&self) -> Self {
        Receiver::from(self.responders.clone())
    }
}

impl<A> Default for Receiver<A> {
    fn default() -> Self {
        Receiver::from(Arc::new(Responders::default()))
    }
}

impl<A: Send> Receiver<A> {
    /// Create a new Receiver.
    pub fn new() -> Receiver<A> {
        Default::default()
    }

    /// Set the response this receiver has to messages. Upon receiving a message
    /// the response will run immediately.
    pub fn respond<F>(self, f: F)
    where
        F: FnMut(&A) + Send + Sync + 'static,
    {
        self.responders.insert(self.k, f);
    }

    /// Set the response this receiver has to messages. Upon receiving a message
    /// the response will run immediately.
    ///
    /// Folds mutably over a Arc<Mutex<T>>.
    pub fn respond_shared<T, F>(self, val: Arc<Mutex<T>>, f: F)
    where
        T: 'static + Send,
        F: Fn(&mut T, &A) + Send + Sync + 'static,
    {
        self.responders.insert(self.k, move |a: &A| {
            let mut t = val.lock().unwrap();
            f(&mut t, a);
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
        B: Send + Sync + 'static,
        X: Into<T>,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> Option<B> + Send + Sync + 'static,
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
    pub fn branch_filter_fold_shared<B, T, F>(&self, state: Arc<Mutex<T>>, f: F) -> Receiver<B>
    where
        B: Send + Sync + 'static,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> Option<B> + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        X: Into<T>,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> B + Send + Sync + 'static,
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
    pub fn branch_fold_shared<B, T, F>(&self, t: Arc<Mutex<T>>, f: F) -> Receiver<B>
    where
        B: Send + Sync + 'static,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> B + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        F: Fn(&A) -> Option<B> + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        F: Fn(&A) -> B + Send + Sync + 'static,
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
    pub fn forward_filter_fold_shared<B, T, F>(self, tx: &Transmitter<B>, var: Arc<Mutex<T>>, f: F)
    where
        B: Send + Sync + 'static,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> Option<B> + Send + Sync + 'static,
    {
        let tx = tx.clone();
        self.respond(move |a: &A| {
            let result = {
                let mut t = var.lock().unwrap();
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
        B: Send + Sync + 'static,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<B> + Send + Sync + 'static,
    {
        let var = Arc::new(Mutex::new(init.into()));
        self.forward_filter_fold_shared(tx, var, f);
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function. All output of the fold
    /// function is sent to the given transmitter.
    pub fn forward_fold<B, X, T, F>(self, tx: &Transmitter<B>, init: X, f: F)
    where
        B: Send + Sync + 'static,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> B + Send + Sync + 'static,
    {
        self.forward_filter_fold(tx, init, move |t: &mut T, a: &A| Some(f(t, a)))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateful fold function, where the state is a shared mutex. All output of
    /// the fold function is sent to the given transmitter.
    pub fn forward_fold_shared<B, T, F>(self, tx: &Transmitter<B>, t: Arc<Mutex<T>>, f: F)
    where
        B: Send + Sync + 'static,
        T: Send + 'static,
        F: Fn(&mut T, &A) -> B + Send + Sync + 'static,
    {
        self.forward_filter_fold_shared(tx, t, move |t: &mut T, a: &A| Some(f(t, a)))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateless map function. If the map function returns `None` for any messages
    /// those messages will *not* be sent to the given transmitter.
    pub fn forward_filter_map<B, F>(self, tx: &Transmitter<B>, f: F)
    where
        B: Send + Sync + 'static,
        F: Fn(&A) -> Option<B> + Send + Sync + 'static,
    {
        self.forward_filter_fold(tx, (), move |&mut (), a| f(a))
    }

    /// Forwards messages on the given receiver to the given transmitter using a
    /// stateless map function. All output of the map function is sent to the given
    /// transmitter.
    pub fn forward_map<B, F>(self, tx: &Transmitter<B>, f: F)
    where
        B: Send + Sync + 'static,
        F: Fn(&A) -> B + Send + Sync + 'static,
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
        B: Send + Sync + 'static,
        T: Send + 'static,
        X: Into<T>,
        F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + Send + Sync + 'static,
        H: Fn(&mut T, &Option<B>) + Send + Sync + 'static,
    {
        let state = new_shared(init.into());
        let cleanup = Arc::new(Box::new(h));
        let tb = tb.clone();
        self.respond(move |a: &A| {
            let may_async = {
                let mut block_state = state.lock().unwrap();
                f(&mut block_state, a)
            };
            may_async.into_iter().for_each(|block: RecvFuture<B>| {
                let tb_clone = tb.clone();
                let state_clone = state.clone();
                let cleanup_clone = cleanup.clone();
                let future = async move {
                    let opt: Option<B> = block.await;
                    opt.iter().for_each(|b| tb_clone.send(&b));
                    let mut inner_state = state_clone.lock().unwrap();
                    cleanup_clone(&mut inner_state, &opt);
                };
                #[cfg(target_arch = "wasm32")]
                wasm_bindgen_futures::spawn_local(future);
                #[cfg(all(not(target_arch = "wasm32"), feature = "async-tokio"))]
                let _ = tokio::task::spawn(future);
            });
        });
    }

    /// Merge all the receivers into one. Any time a message is received on any
    /// receiver, it will be sent to the returned receiver.
    pub fn merge<B>(rxs: Vec<Receiver<B>>) -> Receiver<B>
    where
        B: Send + Sync + 'static,
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
        A: Clone + 'static,
    {
        let var: Arc<Mutex<Option<A>>> = Default::default();
        let var2: Arc<Mutex<Option<A>>> = var.clone();
        let waker: Arc<Mutex<Option<Waker>>> = Default::default();
        let waker2 = waker.clone();
        self.branch().respond(move |msg| {
            {
                let mut guard = var2.lock().unwrap();
                *guard = Some(msg.clone());
            }
            let mut guard = waker2.lock().unwrap();
            guard.take().into_iter().for_each(|waker| waker.wake());
        });

        MessageFuture { var, waker }
    }
}

/// Create a linked `Transmitter<A>` and `Receiver<A>` pair.
pub fn channel<A>() -> (Transmitter<A>, Receiver<A>)
where
    A: Send + Sync + 'static,
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
    A: Send + Sync + 'static,
    B: Send + Sync + 'static,
    T: Send + 'static,
    F: Fn(&mut T, &A) -> Option<B> + Send + Sync + 'static,
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
    var: Arc<Mutex<T>>,
    f: F,
) -> (Transmitter<A>, Receiver<B>)
where
    A: Send + Sync + 'static,
    B: Send + Sync + 'static,
    T: Send + 'static,
    F: Fn(&mut T, &A) -> Option<B> + Send + Sync + 'static,
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
    A: Send + Sync + 'static,
    B: Send + Sync + 'static,
    T: Send + 'static,
    F: Fn(&mut T, &A) -> B + Send + Sync + 'static,
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
pub fn channel_fold_shared<A, B, T, F>(t: Arc<Mutex<T>>, f: F) -> (Transmitter<A>, Receiver<B>)
where
    A: Send + Sync + 'static,
    B: Send + Sync + 'static,
    T: Send + 'static,
    F: Fn(&mut T, &A) -> B + Send + Sync + 'static,
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
    A: Send + Sync + 'static,
    B: Send + Sync + 'static,
    F: Fn(&A) -> Option<B> + Send + Sync + 'static,
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
    A: Send + Sync + 'static,
    B: Send + Sync + 'static,
    F: Fn(&A) -> B + Send + Sync + 'static,
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
pub fn new_shared<A: 'static, X: Into<A>>(init: X) -> Arc<Mutex<A>> {
    Arc::new(Mutex::new(init.into()))
}

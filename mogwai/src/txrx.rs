//! Instant channels. Just add water ;)
//!
//! Mostly a re-export of the [mogwai_chan] crate.
use std::{cell::RefCell, future::Future, rc::Rc};

pub use mogwai_chan::*;

#[cfg(not(target_arch = "wasm32"))]
use log::warn;

/// Provides asyncronous send and fold for mogwai's [`Transmitter`].
pub trait TransmitterAsync {
    /// Channel input.
    type Input;

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
    fn send_async<FutureA>(&self, fa: FutureA)
    where
        FutureA: Future<Output = Self::Input> + 'static;

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
    fn wire_filter_fold_async<T, B, X, F, H>(&self, rb: &Receiver<B>, init: X, f: F, h: H)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &Self::Input) -> Option<RecvFuture<B>> + 'static,
        H: Fn(&mut T, &Option<B>) + 'static;
}

impl<A: 'static> TransmitterAsync for Transmitter<A> {
    type Input = A;
    #[cfg(not(target_arch = "wasm32"))]
    fn send_async<FutureA>(&self, fa: FutureA)
    where
        FutureA: Future<Output = A> + 'static,
    {
        warn!("Transmitter::send_async is a noop on non-wasm32 targets");
        let _ = fa; // noop
    }
    #[cfg(target_arch = "wasm32")]
    fn send_async<FutureA>(&self, fa: FutureA)
    where
        FutureA: Future<Output = A> + 'static,
    {
        let tx = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let a: A = fa.await;
            tx.send(&a);
        });
    }

    fn wire_filter_fold_async<T, B, X, F, H>(&self, rb: &Receiver<B>, init: X, f: F, h: H)
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

/// Provides asyncronous fold for mogwai [`Receiver`]s.
pub trait ReceiverAsync {
    /// Channel output.
    type Output;

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
    fn forward_filter_fold_async<T, B, X, F, H>(self, tb: &Transmitter<B>, init: X, f: F, h: H)
    where
        B: 'static,
        T: 'static,
        X: Into<T>,
        F: Fn(&mut T, &Self::Output) -> Option<RecvFuture<B>> + 'static,
        H: Fn(&mut T, &Option<B>) + 'static;
}

impl<A> ReceiverAsync for Receiver<A> {
    type Output = A;

    fn forward_filter_fold_async<T, B, X, F, H>(self, tb: &Transmitter<B>, init: X, f: F, h: H)
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
                wasm_bindgen_futures::spawn_local(future);
            });
        });
    }
}

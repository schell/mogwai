use std::sync::{Arc, Mutex};
use std::future::Future;
use std::any::Any;
use std::pin::Pin;
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;

type RecvResponders<A> = Arc<Mutex<HashMap<usize, Box<dyn FnMut(&A)>>>>;


pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<A>>>>;


pub fn wrap_future<A, F>(future:F) -> Option<RecvFuture<A>>
where
  F: Future<Output = Option<A>> + 'static
{
  Some(Box::pin(future))
}


fn recv_from<A>(
  next_k: Arc<Mutex<usize>>,
  branches: RecvResponders<A>
) -> Receiver<A> {
  let k = {
    let mut next_k =
      next_k
      .try_lock()
      .expect("Could not try_lock Transmitter::new_recv");
    let k = *next_k;
    *next_k += 1;
    k
  };

  Receiver {
    k,
    next_k: next_k.clone(),
    branches: branches.clone()
  }
}


pub struct Transmitter<A> {
  next_k: Arc<Mutex<usize>>,
  branches: Arc<Mutex<HashMap<usize, Box<dyn FnMut(&A)>>>>,
}


impl<A> Clone for Transmitter<A> {
  fn clone(&self) -> Self {
    Transmitter {
      next_k: self.next_k.clone(),
      branches: self.branches.clone()
    }
  }
}


impl<A> Transmitter<A> {
  pub fn new() -> Transmitter<A> {
    Transmitter {
      next_k: Arc::new(Mutex::new(0)),
      branches: Arc::new(Mutex::new(HashMap::new()))
    }
  }

  pub fn spawn_recv(&mut self) -> Receiver<A> {
    recv_from(self.next_k.clone(), self.branches.clone())
  }

  pub fn send(&self, a:&A) {
    let mut branches =
      self
      .branches
      .try_lock()
      .expect("Could not get Transmitter lookup");
    branches
      .iter_mut()
      .for_each(|(_, f)| {
        f(a);
      });
  }

  /// Wires the transmitter to send to the given receiver using a stateful fold
  /// function.
  ///
  /// The fold function returns an `Option<B>`. In the case that the value of
  /// `Option<B>` is `None`, no message will be sent to the receiver.
  pub fn wire_filter_fold<T, B, X, F>(&mut self, rb: &Receiver<B>, init:X, f:F)
  where
    B: Any,
    T: Any,
    X: Into<T>,
    F: Fn(&mut T, &A) -> Option<B> + 'static
  {
    let tb = rb.new_trns();
    let ra = self.spawn_recv();
    ra.forward_filter_fold(&tb, init, f);
  }

  /// Wires the transmitter to send to the given receiver using a stateful fold
  /// function.
  pub fn wire_fold<T, B, X, F>(&mut self, rb: &Receiver<B>, init:X, f:F)
  where
    B: Any,
    T: Any,
    X: Into<T>,
    F: Fn(&mut T, &A) -> B + 'static
  {
    let tb = rb.new_trns();
    let ra = self.spawn_recv();
    ra.forward_fold(&tb, init, f);
  }

  /// Wires the transmitter to the given receiver using a stateless map function.
  /// If the map function returns `None` for any messages those messages will
  /// *not* be sent to the given transmitter.
  pub fn wire_filter_map<B, F>(&mut self, rb: &Receiver<B>, f:F)
  where
    B: Any,
    F: Fn(&A) -> Option<B> + 'static
  {
    let tb = rb.new_trns();
    let ra = self.spawn_recv();
    ra.forward_filter_map(&tb, f);
  }

  /// Wires the transmitter to the given receiver using a stateless map function.
  pub fn wire_map<B, F>(&mut self, rb: &Receiver<B>, f:F)
  where
    B: Any,
    F: Fn(&A) -> B + 'static
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
  ///
  /// After the async block returns
  pub fn wire_filter_fold_async<T, B, X, F, H>(
    &mut self,
    rb: &Receiver<B>,
    init:X,
    f:F,
    h:H
  )
  where
    B: Any,
    T: Any,
    X: Into<T>,
    F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + 'static,
    H: Fn(&mut T, &Option<B>) + 'static
  {
    let tb = rb.new_trns();
    let ra = self.spawn_recv();
    ra.forward_filter_fold_async(tb, init, f, h);
  }
}


pub struct Receiver<A> {
  k: usize,
  next_k: Arc<Mutex<usize>>,
  branches: Arc<Mutex<HashMap<usize, Box<dyn FnMut(&A)>>>>,
}


/// Clone a receiver.
/// Be careful with this function. Because of magic, calling `responder` on a
/// clone of a receiver sets the responder for both of those receivers.
/// For most cases if you need a new receiver that receives from the same
/// transmitter you can use `branch`.
pub fn hand_clone<A>(rx: &Receiver<A>) -> Receiver<A> {
  Receiver {
    k: rx.k,
    next_k: rx.next_k.clone(),
    branches: rx.branches.clone()
  }
}


impl<A> Receiver<A> {
  pub fn new() -> Receiver<A> {
    Receiver {
      k: 0,
      next_k: Arc::new(Mutex::new(1)),
      branches: Arc::new(Mutex::new(HashMap::new()))
    }
  }

  /// Set the response this receiver has to messages. Upon receiving a message
  /// the response will run immediately.
  ///
  /// NOTE: Clones of receivers share one response function. This means if you
  /// `respond` on a clone of `recv`, `recv`'s responder will be updated
  /// as well. *Under the hood they are the same responder*.
  /// If you want a new receiver that receives messages from the same transmitter
  /// but has its own responder, use Receiver::branch, not clone.
  pub fn respond<F>(self, f:F)
  where
    F: FnMut(&A) + 'static
  {
    let k = self.k;
    let mut branches =
      self
      .branches
      .try_lock()
      .expect("Could not try_lock Receiver::respond");
    branches.insert(k, Box::new(f));
  }

  /// Removes the responder from the receiver.
  /// This drops anything owned by the responder.
  pub fn drop_responder(&mut self) {
    let mut branches =
      self
      .branches
      .try_lock()
      .expect("Could not try_lock Receiver::drop_responder");
    let _ = branches.remove(&self.k);
  }

  pub fn new_trns(&self) -> Transmitter<A> {
    Transmitter {
      next_k: self.next_k.clone(),
      branches: self.branches.clone()
    }
  }

  /// Branch a receiver off of the original.
  /// Each branch will receive from the same transmitter.
  /// The new branch has no initial response to messages.
  pub fn branch(&self) -> Receiver<A> {
    recv_from(self.next_k.clone(), self.branches.clone())
  }

  /// Branch a new receiver off of an original and wire any messages sent to the
  /// original by using a stateful fold function.
  ///
  /// The fold function returns an `Option<B>`. In the case that the value of
  /// `Option<B>` is `None`, no message will be sent to the new receiver.
  ///
  /// Each branch will receive from the same transmitter.
  pub fn branch_filter_fold<B, X, T, F>(&self, init:X, f:F) -> Receiver<B>
  where
    B: Any,
    X: Into<T>,
    T: Any,
    F: Fn(&mut T, &A) -> Option<B> + 'static
  {
    let ra = self.branch();
    let (tb, rb) = txrx();
    ra.forward_filter_fold(&tb, init, f);
    rb
  }

  /// Branch a new receiver off of an original and wire any messages sent to the
  /// original by using a stateful fold function.
  ///
  /// All output of the fold function is sent to the new receiver.
  ///
  /// Each branch will receive from the same transmitter.
  pub fn branch_fold<B, X, T, F>(&self, init:X, f:F) -> Receiver<B>
  where
    B: Any,
    X: Into<T>,
    T: Any,
    F: Fn(&mut T, &A) -> B + 'static
  {
    let ra = self.branch();
    let (tb, rb) = txrx();
    ra.forward_fold(&tb, init, f);
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
  pub fn branch_filter_map<B, F>(&self, f:F) -> Receiver<B>
  where
    B: Any,
    F: Fn(&A) -> Option<B> + 'static
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
  pub fn branch_map<B, F>(&self, f:F) -> Receiver<B>
  where
    B: Any,
    F: Fn(&A) -> B + 'static
  {
    let ra = self.branch();
    let (tb, rb) = txrx();
    ra.forward_map(&tb, f);
    rb
  }

  /// Forwards messages on the given receiver to the given transmitter using a
  /// stateful fold function.
  ///
  /// The fold function returns an `Option<B>`. In the case that the value of
  /// `Option<B>` is `None`, no message will be sent to the transmitter.
  pub fn forward_filter_fold<B, X, T, F>(self, tx: &Transmitter<B>, init:X, f:F)
  where
    B: Any,
    T: Any,
    X: Into<T>,
    F: Fn(&mut T, &A) -> Option<B> + 'static
  {
    let mut state = init.into();
    let tx = tx.clone();
    self.respond(move |a:&A| {
      let may_msg = f(&mut state, a);
      may_msg
        .iter()
        .for_each(|b:&B| {
          tx.send(b);
        });
    })
  }

  /// Forwards messages on the given receiver to the given transmitter using a
  /// stateful fold function. All output of the fold
  /// function is sent to the given transmitter.
  pub fn forward_fold<B, X, T, F>(self, tx: &Transmitter<B>, init:X, f:F)
  where
    B: Any,
    T: Any,
    X: Into<T>,
    F: Fn(&mut T, &A) -> B + 'static
  {
    self.forward_filter_fold(tx, init, move |t:&mut T, a:&A| {
      Some(f(t, a))
    })
  }

  /// Forwards messages on the given receiver to the given transmitter using a
  /// stateless map function. If the map function returns `None` for any messages
  /// those messages will *not* be sent to the given transmitter.
  pub fn forward_filter_map<B, F>(self, tx: &Transmitter<B>, f:F)
  where
    B: Any,
    F: Fn(&A) -> Option<B> + 'static
  {
    self.forward_filter_fold(tx, (), move |&mut (), a| f(a))
  }

  /// Forwards messages on the given receiver to the given transmitter using a
  /// stateless map function. All output of the map function is sent to the given
  /// transmitter.
  pub fn forward_map<B, F>(self, tx: &Transmitter<B>, f:F)
  where
    B: Any,
    F: Fn(&A) -> B + 'static
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
  pub fn forward_filter_fold_async<T, B, X, F, H>(
    self,
    tb: Transmitter<B>,
    init:X,
    f:F,
    h:H
  )
  where
    B: Any,
    T: Any,
    X: Into<T>,
    F: Fn(&mut T, &A) -> Option<RecvFuture<B>> + 'static,
    H: Fn(&mut T, &Option<B>) + 'static
  {
    let state = Arc::new(Mutex::new(init.into()));
    let cleanup = Arc::new(Box::new(h));
    self.respond(move |a:&A| {
      let may_async = {
        let mut block_state =
          state
          .try_lock()
          .expect("Could not try_lock in Receiver::forward_filter_fold_async for block_state");
        f(&mut block_state, a)
      };
      may_async
        .into_iter()
        .for_each(|block:RecvFuture<B>| {
          let tb_clone = tb.clone();
          let state_clone = state.clone();
          let cleanup_clone = cleanup.clone();
          let future =
            async move {
              let opt:Option<B> =
                block.await;
              opt
                .iter()
                .for_each(|b| tb_clone.send(&b));
              let mut inner_state =
                state_clone
                .try_lock()
                .expect("Could not try_lock Receiver::forward_filter_fold_async for inner_state");
              cleanup_clone(&mut inner_state, &opt);
            };
          spawn_local(future);

        });
    });
  }

  /// Merge all the receivers into one. Any time a message is received on any
  /// receiver, it will be sent to the returned receiver.
  pub fn merge<B:Any>(rxs: Vec<Receiver<B>>) -> Receiver<B> {
    let (tx, rx) = txrx();
    rxs
      .into_iter()
      .for_each(|rx_inc| {
        let tx = tx.clone();
        rx_inc
          .branch()
          .respond(move |a| {
            tx.send(a);
          });
      });
    rx
  }
}

// TODO: Remove Clone impls from tx and rx

pub fn recv<A>() -> Receiver<A> {
  Receiver::new()
}


pub fn trns<A>() -> Transmitter<A> {
  Transmitter::new()
}


pub fn txrx<A>() -> (Transmitter<A>, Receiver<A>) {
  let mut trns = Transmitter::new();
  let recv = trns.spawn_recv();
  (trns, recv)
}


pub fn txrx_filter_fold<A, B, T, F>(t:T, f:F) -> (Transmitter<A>, Receiver<B>)
where
  A:Any,
  B:Any,
  T:Any,
  F:Fn(&mut T, &A) -> Option<B> + 'static,
{
  let (ta, ra) = txrx();
  let (tb, rb) = txrx();
  ra.forward_filter_fold(&tb, t, f);
  (ta, rb)
}


pub fn txrx_fold<A, B, T, F>(t:T, f:F) -> (Transmitter<A>, Receiver<B>)
where
  A:Any,
  B:Any,
  T:Any,
  F:Fn(&mut T, &A) -> B + 'static,
{
  let (ta, ra) = txrx();
  let (tb, rb) = txrx();
  ra.forward_fold(&tb, t, f);
  (ta, rb)
}


pub fn txrx_filter_map<A, B, F>(f:F) -> (Transmitter<A>, Receiver<B>)
where
  A:Any,
  B:Any,
  F:Fn(&A) -> Option<B> + 'static,
{
  let (ta, ra) = txrx();
  let (tb, rb) = txrx();
  ra.forward_filter_map(&tb, f);
  (ta, rb)
}

pub fn txrx_map<A, B, F>(f:F) -> (Transmitter<A>, Receiver<B>)
where
  A:Any,
  B:Any,
  F:Fn(&A) -> B + 'static,
{
  let (ta, ra) = txrx();
  let (tb, rb) = txrx();
  ra.forward_map(&tb, f);
  (ta, rb)
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
    let count = Arc::new(Mutex::new(0));
    let (tx_unit, rx_unit) = txrx::<()>();
    let (tx_i32, rx_i32) = txrx::<i32>();
    {
      let my_count = count.clone();
      rx_i32.respond(move |n:&i32| {
        println!("Got message: {:?}", n);
        my_count
          .try_lock()
          .into_iter()
          .for_each(|mut c| *c = *n);
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

    let final_count:i32 =
      *count
      .try_lock()
      .expect("Could not get final count");

    assert_eq!(final_count, 3);
  }

  #[test]
  fn wire_txrx() {
    let mut tx_unit = Transmitter::<()>::new();
    let rx_str = Receiver::<String>::new();
    tx_unit.wire_filter_fold(&rx_str, 0, |n:&mut i32, &()| -> Option<String> {
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
      remote_got_called
        .try_lock()
        .into_iter()
        .for_each(|mut called| *called = true);
    });

    tx_unit.send(&());
    tx_unit.send(&());
    tx_unit.send(&());

    let ever_called =
      got_called
      .try_lock()
      .map(|t| *t)
      .unwrap_or(false);

    assert!(ever_called);
  }

  #[test]
  fn branch_map() {
    let (tx, rx) = txrx::<()>();
    let ry:Receiver<i32> =
      rx.branch_map(|_| 0);

    let done =
      Arc::new(Mutex::new(false));

    let cdone = done.clone();
    ry.respond(move |n| {
      if *n == 0 {
        *cdone
          .try_lock()
          .unwrap()
          = true;
      }
    });

    tx.send(&());

    assert!(*done.try_lock().unwrap());
  }
}

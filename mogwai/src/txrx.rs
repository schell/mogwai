use std::sync::{Arc, Mutex};
use std::future::Future;
use std::any::Any;
use std::pin::Pin;
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;
pub use wasm_bindgen_futures::JsFuture;

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

  /// Wires the transmitter to the given receiver using a stateful fold function.
  pub fn wire_fold<T, B, X, F>(&mut self, rb: &Receiver<B>, init:X, f:F)
  where
    B: Any,
    T: Any + Send + Sync,
    X: Into<T>,
    F: Fn(&T, &A) -> (T, Option<B>) + Send + Sync + 'static
  {
    let tb = rb.new_trns();
    let mut ra = self.spawn_recv();
    ra.forward_fold(tb, init, f);
  }

  /// Wires the transmitter to the given receiver asynchronously using a stateful
  /// fold function.
  pub fn wire_fold_async<T, B, X, F, H>(ta: &mut Transmitter<A>, rb: &Receiver<B>, init:X, f:F, h:H)
  where
    B: Any,
    T: Any + Send + Sync,
    X: Into<T>,
    F: Fn(&T, &A) -> (T, Option<RecvFuture<B>>) + 'static,
    H: Fn(&T, &B) -> T + 'static
  {
    let tb = rb.new_trns();
    let mut ra = ta.spawn_recv();
    ra.forward_fold_async(tb, init, f, h);
  }

  /// Wires the transmitter to the given receiver using a stateless map function.
  pub fn wire_map<B, X, F>(&mut self, rb: &Receiver<B>, f:F)
  where
    B: Any,
    F: Fn(&A) -> Option<B> + Send + Sync + 'static
  {
    let tb = rb.new_trns();
    let mut ra = self.spawn_recv();
    ra.forward_map(tb, f);
  }
}


#[derive(Clone)]
pub struct Receiver<A> {
  k: usize,
  next_k: Arc<Mutex<usize>>,
  branches: Arc<Mutex<HashMap<usize, Box<dyn FnMut(&A)>>>>,
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
  /// `set_responder` on a clone of `recv`, `recv`'s responder will be updated
  /// as well. *Under the hood they are the same responder*.
  /// If you want a new receiver that receives messages from the same transmitter
  /// but has its own responder, use Receiver::branch, not clone.
  pub fn set_responder<F>(&mut self, f:F)
  where
    F: FnMut(&A) + 'static
  {
    let k = self.k;
    let mut branches =
      self
      .branches
      .try_lock()
      .expect("Could not try_lock Receiver::set_responder");
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

  /// Forwards messages on the given receiver to the given transmitter using a
  /// stateful fold function.
  /// NOTE: Overwrites this receiver's responder.
  pub fn forward_fold<T, B, X, F>(&mut self, tx: Transmitter<B>, init:X, f:F)
  where
    B: Any,
    T: Any + Send + Sync,
    X: Into<T>,
    F: Fn(&T, &A) -> (T, Option<B>) + Send + Sync + 'static
  {
    let mut state = init.into();
    self.set_responder(move |a:&A| {
      let (new_state, may_msg) = f(&state, a);
      state = new_state;
      may_msg
        .iter()
        .for_each(|b:&B| {
          tx.send(b);
        });
    })
  }

  /// Forwards messages on the given receiver to the given transmitter using a
  /// stateless map function.
  pub fn forward_map<B, F>(&mut self, tx: Transmitter<B>, f:F)
  where
    B: Any,
    F: Fn(&A) -> Option<B> + Send + Sync + 'static
  {
    self
      .forward_fold(
        tx,
        (),
        move |&(), a| {
          ((), f(a))
        }
      )
  }

  /// Branch a receiver off of the original and map any messages using a map
  /// function.
  /// Each branch will receive from the same transmitter.
  /// The new branch has no initial response to messages.
  pub fn branch_map<B:Any, F>(&self, f:F) -> Receiver<B>
  where
    F: Fn(&A) -> Option<B> + Send + Sync + 'static
  {
    let (tb, rb) = terminals::<B>();
    let mut ra = self.branch();
    ra.forward_map(tb, f);
    rb
  }

  pub fn forward_fold_async<T, B, X, F, H>(&mut self, tb: Transmitter<B>, init:X, f:F, h:H)
  where
    B: Any,
    T: Any + Send + Sync,
    X: Into<T>,
    F: Fn(&T, &A) -> (T, Option<RecvFuture<B>>) + 'static,
    H: Fn(&T, &B) -> T + 'static
  {
    let state = Arc::new(Mutex::new(init.into()));
    let cleanup = Arc::new(Box::new(h));
    self.set_responder(move |a:&A| {
      let may_async = {
        let mut block_state =
          state
          .try_lock()
          .expect("Could not try_lock in Receiver::forward_fold_async for block_state");
        // Update the shared state.
        let (new_state, may_async) = f(&block_state, a);
        *block_state = new_state;

        may_async
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
              trace!("sending async responder message");
              opt
                .into_iter()
                .for_each(|b:B| {
                  let mut inner_state =
                    state_clone
                    .try_lock()
                    .expect("Could not try_lock Receiver::forward_fold_async for inner_state");
                  *inner_state =
                    cleanup_clone(&inner_state, &b);
                  tb_clone.send(&b);
                });
            };
          spawn_local(future);
          trace!("spawned async responder");
        });
    });
  }
}


pub fn terminals<A>() -> (Transmitter<A>, Receiver<A>) {
  let mut trns = Transmitter::new();
  let recv = trns.spawn_recv();
  (trns, recv)
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
  fn txrx() {
    let count = Arc::new(Mutex::new(0));
    let (tx_unit, mut rx_unit) = terminals::<()>();
    let (tx_i32, mut rx_i32) = terminals::<i32>();
    {
      let my_count = count.clone();
      rx_i32.set_responder(move |n:&i32| {
        println!("Got message: {:?}", n);
        my_count
          .try_lock()
          .into_iter()
          .for_each(|mut c| *c = *n);
      });

      let mut n = 0;
      rx_unit.set_responder(move |()| {
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
    let mut rx_str = Receiver::<String>::new();
    tx_unit.wire_fold(&rx_str, 0, |n:&i32, &()| -> (i32, Option<String>) {
      let next = n + 1;
      let should_tx = next >= 3;
      let may_msg =
        if should_tx {
          Some(format!("Passed 3 incoming messages ({})", next))
        } else {
          None
        };
      (next, may_msg)
    });

    let got_called = Arc::new(Mutex::new(false));
    let remote_got_called = got_called.clone();
    rx_str.set_responder(move |s: &String| {
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
    let (tx, rx) = terminals::<()>();
    let mut ry:Receiver<i32> =
      rx.branch_map(|_| Some(0));

    let done =
      Arc::new(Mutex::new(false));

    let cdone = done.clone();
    ry.set_responder(move |n| {
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

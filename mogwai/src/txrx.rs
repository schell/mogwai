use std::sync::{Arc, Mutex};
use std::any::Any;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Transmitter<A> {
  next_k: Arc<Mutex<usize>>,
  branches: Arc<Mutex<HashMap<usize, Box<dyn FnMut(&A)>>>>,
}


impl<A> Transmitter<A> {
  pub fn new() -> Transmitter<A> {
    Transmitter {
      next_k: Arc::new(Mutex::new(0)),
      branches: Arc::new(Mutex::new(HashMap::new()))
    }
  }

  pub fn spawn_recv(&mut self) -> Receiver<A> {
    let k = {
      let mut next_k =
        self
        .next_k
        .try_lock()
        .expect("Could not try_lock Transmitter::new_recv");
      let k = *next_k;
      *next_k += 1;
      k
    };

    Receiver {
      k,
      next_k: self.next_k.clone(),
      branches: self.branches.clone()
    }
  }

  pub fn send(&mut self, a:&A) {
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

  pub fn new_trns(&self) -> Transmitter<A> {
    Transmitter {
      next_k: self.next_k.clone(),
      branches: self.branches.clone()
    }
  }
}


pub fn instant_terminals<A>() -> (Transmitter<A>, Receiver<A>) {
  let mut trns = Transmitter::new();
  let recv = trns.spawn_recv();
  (trns, recv)
}


pub fn wire<A, T, B, X:, F>(tx: &mut Transmitter<A>, rx: &Receiver<B>, init:X, f:F)
where
  B: Any,
  T: Any + Send + Sync,
  X: Into<T>,
  F: Fn(&T, &A) -> (T, Option<B>) + Send + Sync + 'static
{
  let mut state = init.into();
  let mut tb = rx.new_trns();
  let mut ra = tx.spawn_recv();
  ra.set_responder(move |a:&A| {
    let (new_state, may_msg) = f(&state, a);
    state = new_state;
    may_msg
      .iter()
      .for_each(|b:&B| {
        tb.send(b);
      });
  })
}


#[cfg(test)]
mod instant_txrx {
  use super::*;

  #[test]
  fn txrx() {
    let count = Arc::new(Mutex::new(0));
    let (mut tx_unit, mut rx_unit) = instant_terminals::<()>();
    let (mut tx_i32, mut rx_i32) = instant_terminals::<i32>();
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
    wire(&mut tx_unit, &mut rx_str, 0, |n:&i32, &()| -> (i32, Option<String>) {
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
}

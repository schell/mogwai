use std::sync::{Arc, Mutex};
use std::any::Any;
use shrev::{ReaderId, EventChannel};
use std::collections::HashMap;

pub use shrev::EventIterator;


/// An input is a terminal to push input into.
pub struct Transmitter<A> {
  events: Arc<Mutex<EventChannel<A>>>
}


impl<A:shrev::Event> Transmitter<A> {
  pub fn new<B:Any + Send + Sync>() -> Transmitter<B> {
   Transmitter {
      events: Arc::new(Mutex::new(EventChannel::new()))
    }
  }

  pub fn push(&mut self, a:A) {
    self
      .events
      .try_lock()
      .expect("Could not try_lock on Transmitter::push::self.events")
      .single_write(a);
  }

  pub fn new_recv(&self) -> Receiver<A> {
    let events = self.events.clone();
    let reader =
      events
      .try_lock()
      .unwrap()
      .register_reader();
    Receiver {
      reader: Some(reader),
      events
    }
  }
}


impl<T> Clone for Transmitter<T> {
  fn clone(&self) -> Transmitter<T> {
    Transmitter {
      events: self.events.clone()
    }
  }
}


/// An output is a terminal to get ouput from.
pub struct Receiver<B:Any> {
  reader: Option<ReaderId<B>>,
  events: Arc<Mutex<EventChannel<B>>>
}


impl<A:shrev::Event + Clone> Receiver<A> {
  pub fn new() -> Receiver<A> {
    let mut events = EventChannel::new();
    let reader = events.register_reader();
    Receiver {
      reader: Some(reader),
      events: Arc::new(Mutex::new(events))
    }
  }

  pub fn read(&mut self) -> Vec<A> {
    let mut guard =
      self
      .events
      .try_lock()
      .expect("Could not try_lock Receiver::pop");

    let mut reader =
      self
      .reader
      .take()
      .unwrap_or(guard.register_reader());

    let iter:EventIterator<A> =
      guard
      .read(&mut reader);

    self.reader = Some(reader);

    let mut items:Vec<A> = vec![];
    for item in iter {
      items.push(item.clone());
    }
    items
  }
  pub fn new_trns(&self) -> Transmitter<A> {
    Transmitter {
      events: self.events.clone()
    }
  }

}


impl<T:shrev::Event> Clone for Receiver<T> {
  fn clone(&self) -> Receiver<T> {
    Receiver {
      reader: None,
      events: self.events.clone()
    }
  }
}


pub fn terminals<A:shrev::Event>() -> (Transmitter<A>, Receiver<A>) {
  let input = Transmitter::<A>::new();
  let output = input.new_recv();
  (input, output)
}


#[cfg(test)]
mod input_output_tests {
  use super::*;

  #[test]
  fn tx_rx_relationship() {
    let (mut tx, mut rx) = terminals::<i32>();
    let mut rx2 = rx.clone();

    tx.push(0);
    tx.push(1);
    tx.push(2);

    let rx_items = rx.read();
    let rx2_items = rx2.read();

    assert_eq!(rx_items.len(), 3);
    assert_eq!(rx2_items.len(), 3);

    assert_eq!(rx_items[0], 0);

    assert_eq!(rx2_items[0], 0);
    assert_eq!(rx2_items[1], 1);
    assert_eq!(rx2_items[2], 2);

    assert_eq!(rx.read().len(), 0);
  }
}

#[derive(Clone)]
pub struct InstantTransmitter<A> {
  next_k: Arc<Mutex<usize>>,
  branches: Arc<Mutex<HashMap<usize, Box<dyn FnMut(&A) + Send + Sync>>>>,
}


impl<A> InstantTransmitter<A> {
  pub fn new() -> InstantTransmitter<A> {
    InstantTransmitter {
      next_k: Arc::new(Mutex::new(0)),
      branches: Arc::new(Mutex::new(HashMap::new()))
    }
  }

  pub fn spawn_recv(&mut self) -> InstantReceiver<A> {
    let k = {
      let mut next_k =
        self
        .next_k
        .try_lock()
        .expect("Could not try_lock InstantTransmitter::new_recv");
      let k = *next_k;
      *next_k += 1;
      k
    };

    InstantReceiver {
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
      .expect("Could not get InstantTransmitter lookup");
    branches
      .iter_mut()
      .for_each(|(_, f)| {
        f(a);
      });
  }
}


#[derive(Clone)]
pub struct InstantReceiver<A> {
  k: usize,
  next_k: Arc<Mutex<usize>>,
  branches: Arc<Mutex<HashMap<usize, Box<dyn FnMut(&A) + Send + Sync>>>>,
}


impl<A> InstantReceiver<A> {
  pub fn new() -> InstantReceiver<A> {
    InstantReceiver {
      k: 0,
      next_k: Arc::new(Mutex::new(1)),
      branches: Arc::new(Mutex::new(HashMap::new()))
    }
  }

  pub fn set_responder<F>(&mut self, f:F)
  where
    F: FnMut(&A) + Send + Sync +'static
  {
    let k = self.k;
    let mut branches =
      self
      .branches
      .try_lock()
      .expect("Could not try_lock InstantReceiver::set_responder");
    branches.insert(k, Box::new(f));
  }

  pub fn new_trns(&self) -> InstantTransmitter<A> {
    InstantTransmitter {
      next_k: self.next_k.clone(),
      branches: self.branches.clone()
    }
  }
}


pub fn instant_terminals<A>() -> (InstantTransmitter<A>, InstantReceiver<A>) {
  let mut trns = InstantTransmitter::new();
  let recv = trns.spawn_recv();
  (trns, recv)
}


pub fn wire<A, T, B, X:, F>(tx: &mut InstantTransmitter<A>, rx: &InstantReceiver<B>, init:X, f:F)
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
    let mut tx_unit = InstantTransmitter::<()>::new();
    let mut rx_str = InstantReceiver::<String>::new();
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

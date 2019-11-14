use std::sync::{Arc, Mutex};


/// An input is a terminal to push input into.
// TODO: Keep track of subscribers so we can empty events.
// That way we don't have to have a "clear" step
pub struct Input<A> {
  next_k: usize,
  events: Arc<Mutex<Vec<(usize, A)>>>
}


impl<A> Input<A> {
  pub fn push(&mut self, a:A) {
    let k = self.next_k;
    self.next_k += 1;

    self
      .events
      .try_lock()
      .expect("Could not try_lock on Input::push::self.events")
      .push((k, a));
  }

  pub fn has_items(&self) -> bool {
    self
      .events
      .try_lock()
      .expect("Could not try_lock on Input::has_items::self.events")
      .is_empty()
      == false
  }

  pub fn clear(&mut self) {
    let mut events =
      self
      .events
      .try_lock()
      .expect("Could not try_lock on Input::clear::self.events");
    *events = vec![];
  }

  pub fn new_output(&self) -> Output<A> {
    Output {
      next_k: self.next_k,
      events: self.events.clone()
    }
  }
}


impl<T> Clone for Input<T> {
  fn clone(&self) -> Input<T> {
    Input {
      next_k: self.next_k,
      events: self.events.clone()
    }
  }
}


#[cfg(test)]
mod input_tests {
  use super::*;


  #[test]
  fn shared_mutex() {
    let d1:Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let d2 = d1.clone();
    {
      *d2
        .as_ref()
        .try_lock()
        .unwrap() = 1;
    }
    assert_eq!(*d1.as_ref().try_lock().unwrap(), 1);
  }


  #[test]
  fn input() {
    let mut input =
      Input {
        next_k: 0,
        events: Arc::new(Mutex::new(vec![]))
      };

    input.push(());
    input.push(());
    assert!(input.has_items());

    input.clear();
    assert_eq!(input.has_items(), false);
  }
}


/// An output is a terminal to get ouput from.
pub struct Output<B> {
  next_k: usize,
  events: Arc<Mutex<Vec<(usize, B)>>>
}


impl<A:Clone> Output<A> {
  pub fn pop(&mut self) -> Option<A> {
    if self.has_items() {
      let items =
        self
        .events
        .try_lock()
        .unwrap();

      let mut found:Option<A> = None;

      'search: for (k, event) in items.iter() {
        if *k < self.next_k {
          continue;
        }
        found = Some(event.clone());
        self.next_k += 1;
        break 'search;
      }
      found
    } else {
      None
    }
  }

  fn last_event_k(&self) -> Option<usize> {
    self
      .events
      .try_lock()
      .expect("Could not try_lock Output::last_event_k::events")
      .last()
      .map(|(k, _)| *k)
  }

  pub fn has_items(&self) -> bool {
    let last_k =
      self
      .last_event_k();

    last_k.is_some() && self.next_k < last_k.unwrap()
  }

  pub fn drain(&mut self) -> Vec<A> {
    println!("drain");
    let mut items = vec![];
    'get_items: loop  {
      println!("  checking items");
      if !self.has_items() {
        break 'get_items;
      }
      println!("  still has items");

      self
        .pop()
        .into_iter()
        .for_each(|i| {
          items.push(i);
        });

      println!("  popped an item");
    }
    println!("  drained items");
    items
  }

  pub fn new_input(&self) -> Input<A> {
    Input {
      next_k: self.next_k,
      events: self.events.clone()
    }
  }

}


impl<T> Clone for Output<T> {
  fn clone(&self) -> Output<T> {
    Output {
      next_k: self.next_k,
      events: self.events.clone()
    }
  }
}


#[cfg(test)]
mod input_output_tests {
  use super::*;

  #[test]
  fn tx_rx_relationship() {
    let (mut tx, mut rx) = terminals();
    let mut rx2 = rx.clone();

    tx.push(());
    assert!(rx.has_items());
    assert!(rx2.has_items());

    rx.pop()
      .expect("Could not receive on rx");

    rx2.pop()
      .expect("rx2 is already drained");
  }
}


pub fn terminals<A>() -> (Input<A>, Output<A>) {
  let input =
    Input {
      next_k: 0,
      events: Arc::new(Mutex::new(vec![]))
    };
  let output =
    input.new_output();
  (input, output)
}


/// A Wire connects a Receiver<A> with a Sender<B> using a fold function F.
#[derive(Clone)]
pub struct Wire<A, B> {
  recv: Output<A>,
  send: Input<B>,
  fold: Arc<Box<dyn Fn(B, A) -> B>>
}


impl<A:Clone, B> Wire<A, B> {
  fn fold_id<X,Y>() -> Arc<Box<Fn(Y, X) -> Y>> {
    Arc::new(
      Box::new(|b, _| b)
    )
  }

  pub fn hookups() -> (Input<A>, Wire<A, B>, Output<B>) {
    let (tx, rx) = terminals();
    let (ty, ry) = terminals();
    let wire =
      Wire {
        recv: rx,
        send: ty,
        fold: Self::fold_id()
      };
    (tx, wire, ry)
  }

  /// Extends an existing wire with new output.
  /// Creates a new wire that uses the same input, but transmits to new output.
  pub fn extend_output<C>(&self) -> (Wire<A, C>, Output<C>) {
    let (tc, rc) = terminals();
    let wire =
      Wire {
        recv: self.recv.clone(),
        send: tc,
        fold: Self::fold_id()
      };
    (wire, rc)
  }

  pub fn extend_input<Z>(&self) -> (Input<Z>, Wire<Z, B>) {
    let (tz, rz) = terminals();
    let wire =
      Wire {
        recv: rz,
        send: self.send.clone(),
        fold: Self::fold_id()
      };
    (tz, wire)
  }

  /// Set the fold function for this wire.
  pub fn fold_with<F:Fn(B, A) -> B + 'static>(&mut self, f:F) {
    self.fold = Arc::new(Box::new(f));
  }

  /// Receive all input, fold it all into one output and transmit it, if possible.
  pub fn run<T:Into<B>>(&mut self, acc: T) {
    println!("running wire\n  checking items");
    if !self.recv.has_items() {
      println!("  wire's recv is empty");
      return;
    }
    println!("  has items");

    let f =
      self
      .fold
      .as_ref();

    let items =
      self
      .recv
      .drain();

    println!("  got {} items", items.len());

    let output =
      items
      .into_iter()
      .fold(
        acc.into(),
        |b:B, a:A| f(b, a)
      );

    println!("  got output");

    self
      .send
      .push(output);
  }
}


#[cfg(test)]
mod wire_tests {
  use super::*;

  #[test]
  fn tx_wire_rx_relationship() {
    // Create new hookups
    let (_tx, mut wire_xy, _ry) = Wire::<i32, i32>::hookups();
    let (mut tw, mut wire_wx) = wire_xy.extend_input::<()>();
    let (mut wire_yz, mut rz) = wire_xy.extend_output::<String>();

    // Explain how the wires accumulate their input into output

    // wire_wx converts a unit into a single count
    wire_wx
      .fold_with(|_, ()| 1);

    // wire_xy takes a single count and sums it
    wire_xy
      .fold_with(|acc, n| acc + n);

    // wire_yz takes a number and turns it into a string
    wire_yz
      .fold_with(|_, n| format!("The number {}", n));

    // Put some things in the pipe!
    println!("sending on tw");
    tw.push(());
    //tw.send(())
    //  .unwrap();
    //tw.send(())
    //  .unwrap();

    println!("running wire_wx");
    wire_wx.run(0);
    println!("running wire_xy");
    wire_xy.run(0);
    println!("running wire_yz");
    wire_yz.run("");

    println!("getting output");
    let output =
      rz
      .pop()
      .expect("Could not get output");

    assert_eq!(output, "The number 3".to_string());
  }
}

use std::sync::Arc;
use std::any::Any;

use super::txrx::*;

/// A Wire connects an input of `A` with an output of `B` using a fold function
/// `F` that accumulates `A`s into a state `S` and sends out `B`s downstream, if
/// possible.
#[derive(Clone)]
pub struct Wire<A:shrev::Event + Clone, S, B:shrev::Event + Clone> {
  state: Option<S>,
  recv: Receiver<A>,
  send: Transmitter<B>,
  fold: Arc<Box<dyn Fn(S, A) -> (S, Option<B>) + Send + Sync>>
}


impl<A:shrev::Event + Clone, S:shrev::Event, B:shrev::Event + Clone> Wire<A, S, B> {
  /// A closure that accumulates nothing and never produces a downstream value.
  pub fn do_nothing() -> Arc<Box<dyn Fn(S, A) -> (S, Option<B>) + Send + Sync>> {
    Arc::new(Box::new(|s, _| (s, None)))
  }

  pub fn hookups<X:Into<S>>(init: X) -> (Transmitter<A>, Wire<A, S, B>, Receiver<B>) {
    let (tx, rx) = terminals();
    let (ty, ry) = terminals();
    let wire =
      Wire {
        state: Some(init.into()),
        recv: rx,
        send: ty,
        fold: Self::do_nothing()
      };
    (tx, wire, ry)
  }

  /// Branch a new wire and output off of an input.
  pub fn branch<X:Into<S>>(tx: Transmitter<A>, init:X) -> (Wire<A, S, B>, Receiver<B>)  {
    let (tb, rb) = terminals();
    let wire =
      Wire {
        state: Some(init.into()),
        recv: tx.new_recv(),
        send: tb,
        fold: Wire::<A, S, B>::do_nothing()
      };
    (wire, rb)
  }

  /// Extends an existing wire with new output.
  /// Creates a new wire that uses the same input, but transmits to new output.
  pub fn extend_output<T:shrev::Event, C:shrev::Event + Clone, X:Into<T>>(&self, init:X) -> (Wire<B, T, C>, Receiver<C>) {
    let (tc, rc) = terminals();
    let wire =
      Wire {
        state: Some(init.into()),
        recv: self.send.new_recv(),
        send: tc,
        fold: Wire::<B, T, C>::do_nothing()
      };
    (wire, rc)
  }

  pub fn extend_input<Z:shrev::Event + Clone, T:shrev::Event, X:Into<T>>(&self, init:X) -> (Transmitter<Z>, Wire<Z, T, A>) {
    let (tz, rz) = terminals();
    let wire =
      Wire {
        state: Some(init.into()),
        recv: rz,
        send: self.recv.new_trns(),
        fold: Wire::<Z, T, A>::do_nothing()
      };
    (tz, wire)
  }

  pub fn between(ta: &Transmitter<A>, state: S, rb: &Receiver<B>) -> Wire<A, S, B> {
    let ra = ta.new_recv();
    let tb = rb.new_trns();
    Wire {
      state: Some(state),
      recv: ra,
      send: tb,
      fold: Wire::do_nothing()
    }
  }

  /// Set the fold function for this wire.
  pub fn on_input<F:Fn(S, A) -> (S, Option<B>) + shrev::Event>(&mut self, f:F) {
    self.fold = Arc::new(Box::new(f));
  }

  /// Consume all input, fold it into internal state using the fold function and
  /// transmit any produced values downstream.
  pub fn run(&mut self) {
    println!("running wire\n  checking items");
    let items:Vec<A> =
      self
      .recv
      .read();

    if items.len() > 0 {
      println!("  wire's recv is empty");
      return;
    }
    println!("  got {} items", items.len());

    let f =
      self
      .fold
      .as_ref();

    let start_state =
      self
      .state
      .take()
      .expect("Wire is missing its internal state");
    let (end_state, outputs):(S, Vec<B>) =
      items
      .into_iter()
      .fold(
        (start_state, vec![]),
        |(state, mut outs), a:A| {
          let (next_state, may_out) =
            f(state, a);
          may_out
            .into_iter()
            .for_each(|b:B| {
              outs.push(b);
            });
          (next_state, outs)
        });

    self.state = Some(end_state);

    println!("  sending {} outputs", outputs.len());

    outputs
      .into_iter()
      .for_each(|b:B| {
        self
          .send
          .push(b);
      });
  }
}


/// A bundle is simply a wrapper around a closure.
#[derive(Clone)]
pub struct Bundle(Arc<Box<FnMut() + Send + Sync>>);


impl Bundle {
  fn run(&mut self) {
    let run:&mut Box<FnMut() + Send + Sync> =
      Arc::get_mut(&mut self.0)
      .expect("Could not get FuseBox bundle");
    trace!("Bundle::run");
    run();
  }
}


impl<A:shrev::Event + Clone, T:Any + Send + Sync, B:shrev::Event + Clone> From<Wire<A, T, B>> for Bundle {
  fn from(wire: Wire<A, T, B>) -> Self {
    let mut wire = wire;
    Bundle(
      Arc::new(
        Box::new(move || {
          wire.run();
        })
      )
    )
  }
}


#[cfg(test)]
mod wire_tests {
  use super::*;

  #[test]
  fn tx_wirexz_rz_relationship() {
    // First we'll create a set of new hookups - a transmitter, a wire and a
    // receiver

    // wire_wx takes () as input, accumulates an i32 and sends each downstream.
    // We don't care about the output because we're going to extend this wire
    // below.
    let (mut tw, mut wire_wx, mut rx) = Wire::<(), i32, i32>::hookups(0);

    // Then we'll extend the wire with another wire that connects to another
    // receiver.

    // wire_xy takes an i32 as input, accumulates () (aka nothing) and sends each
    // downstream
    let (mut wire_xy, mut ry) = wire_wx.extend_output(());

    // Then we'll extend the wire a bit further with yet another wire and
    // receiver pair.

    // wire_yz takes an i32 as input, accumulates () (again, nothing) and
    // converts the input i32 into an output string, producing it downstream
    let (mut wire_yz, mut rz) = wire_xy.extend_output::<(), String, _>(());

    // Now we can define how the wires accumulate their input into state and
    // produce output.

    // wire_wx counts each input by incrementing a counter and producing that
    // counter on each input
    wire_wx
      .on_input(|prev:i32, ()| {
        println!("folding wire_wx {} ()", prev);
        let next = prev + 1;
        (next, Some(next))
      });

    // wire_xy has no internal state and multiplies input numbers by 2, producing
    // on each input.
    wire_xy
      .on_input(|(), n:i32| {
        println!("folding wire_xy () {}", n);
        ((), Some(n * 2))
      });

    // wire_yz has no internal state and converts input numbers into output
    // strings - producing them for every input.
    wire_yz
      .on_input(|(), n:i32| {
        println!("folding wire_yz () {}", n);
        ((), Some(format!("The number {}", n)))
      });

    // Put some things in the pipe!
    println!("sending on tw");
    tw.push(());
    tw.push(());
    tw.push(());

    println!("running wire_wx");
    wire_wx.run();

    // We can test the output of each wire individually by popping from their
    // associated receivers. We can do this without disturbing the chain.
    // Alternatively we could have dropped these receivers if not needed.
    //let rx_items:Vec<&i32> =
    //  rx
    //  .read()
    //  .collect::<Vec<_>>()
    //  .to_owned();
    //assert_eq!(rx.pop(), Some(1));
    //assert_eq!(rx.pop(), Some(2));
    //assert_eq!(rx.pop(), Some(3));

    //println!("running wire_xy");
    //wire_xy.run();

    //assert_eq!(ry.pop(), Some(2));
    //assert_eq!(ry.pop(), Some(4));
    //assert_eq!(ry.pop(), Some(6));

    //println!("running wire_yz");
    //wire_yz.run();

    //println!("getting final output");
    //assert_eq!(rz.pop(), Some("The number 2".into()));
    //assert_eq!(rz.pop(), Some("The number 4".into()));
    //assert_eq!(rz.pop(), Some("The number 6".into()));

    //assert_eq!(rx.has_items(), false);
    //assert_eq!(ry.has_items(), false);
    //assert_eq!(rz.has_items(), false);
  }
}


#[cfg(test)]
mod fuse_concept {
  use super::*;

  #[test]
  // We want to be able to erase the A, B types from a connection of wires.
  // That way they can all be stored together inside a Gizmo and run when need
  // be.
  fn fuse_concept() {
    let (mut tx, mut run_fused, mut rz) = {
      // First we'll create two connected wires.
      // wire_xy will send one unit dowstream for every three it gets from upstream
      let (tx, mut wire_xy, _) = Wire::<(), i32, ()>::hookups(0);
      wire_xy
        .on_input(|n, ()| {
          let next = n + 1;

          if next >= 3 {
            (0, Some(()))
          } else {
            (next, None)
          }
        });

      // wire_yz will take units as input, counting each one and sending a String
      // downstream each time.
      let (mut wire_yz, rz) = wire_xy.extend_output::<i32, String, _>(0);
      wire_yz
        .on_input(|n, ()| {
          let next = n + 1;

          (next, Some(format!("Downstream saw {} inputs", next)))
        });

      // In order to bundle up any number of wires we simply need a way to reduce
      // two wires of <A, B> and <B, C> into one wire of <A, C>. Then we can fuse
      // that with another of <C, D> and so forth.

      // We should be able to do this with a closure that takes ownership of the
      // two wires. Let's try a Fn with its own mutable state variable.
      let fused_wires = move || {
        wire_xy.run();
        wire_yz.run();
      };
      (tx, fused_wires, rz)
    };

    tx.push(());
    tx.push(());
    tx.push(());
    tx.push(());
    tx.push(());
    tx.push(());
    run_fused();

    let items:Vec<String> =
      rz.read();

    assert_eq!(items.len(), 2);

    let first =
      items
      .first()
      .unwrap()
      .clone();
    assert_eq!(first, "Downstream saw 2 inputs".to_string());
  }
}


#[derive(Clone)]
pub struct FuseBox {
  bundles: Vec<Bundle>
}


impl FuseBox {
  pub fn new() -> FuseBox {
    FuseBox {
      bundles: vec![]
    }
  }

  pub fn bundle<I:Into<Bundle>>(&mut self, thing: I) {
    self
      .bundles
      .push(thing.into())
  }

  pub fn with<I:Into<Bundle>>(self, thing: I) -> Self {
    let mut fb = self;
    fb.bundle(thing);
    fb
  }

  pub fn run(&mut self) {
    trace!("FuseBox::run {} bundles", self.bundles.len());
    self
      .bundles
      .iter_mut()
      .for_each(|bundle: &mut Bundle| {
        trace!("FuseBox::run bundle");
        bundle.run();
      });
  }

  /// The number of bundles in this fusebox.
  pub fn len(&self) -> usize {
    self.bundles.len()
  }
}

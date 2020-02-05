//! A simple component without internal state.
//!
//! This component has no internal state, uses only one message type and forwards
//! all incoming model messages to its view. It's useful when you have a small
//! component that needs a little communication with an owner, but doesn't need
//! its own state.
use std::any::Any;
use super::{Component};
use super::subscriber::Subscriber;
use super::super::txrx::{Transmitter, Receiver};
use super::super::builder::{GizmoBuilder};


pub struct SimpleComponent<T>(Box<dyn Fn(Transmitter<T>, Receiver<T>) -> GizmoBuilder>);


impl<T:Any + Clone> SimpleComponent<T> {
  pub fn new<F>(builder:F) -> SimpleComponent<T>
  where
    F: Fn(Transmitter<T>, Receiver<T>) -> GizmoBuilder + 'static
  {
    SimpleComponent(Box::new(builder))
  }
}


impl<T:Any + Clone> Component for SimpleComponent<T> {
  type ModelMsg = T;
  type ViewMsg = T;

  fn update(
    &mut self,
    msg: &T,
    tx_view: &Transmitter<T>,
    _sub: &Subscriber<T>
  ) {
    tx_view.send(msg);
  }

  fn builder(
    &self,
    tx: Transmitter<T>,
    rx: Receiver<T>
  ) -> GizmoBuilder {
    self.0(tx, rx)
  }
}

//! A very limited transmitter used to map messages.
use super::super::txrx::{Receiver, Transmitter};
use std::future::Future;


/// A subscriber allows a component to subscribe to other components' messages
/// without having explicit access to both Transmitter and Receiver. This allows
/// the parent component to map child component messages into its own updates
/// without needing its own transmitter. This is good because if `send` is called
/// on a component's own ModelMsg transmitter during its Component::update it
/// triggers a lock contetion. So a subscriber allows forwarding and wiring
/// without enabling sending.
#[derive(Clone)]
pub struct Subscriber<Msg> {
    tx: Transmitter<Msg>,
}


impl<Msg: Clone + 'static> Subscriber<Msg> {
    pub fn new(tx: &Transmitter<Msg>) -> Subscriber<Msg> {
        Subscriber { tx: tx.clone() }
    }

    /// Subscribe to a receiver by forwarding messages from it using a filter map
    /// function.
    pub fn subscribe_filter_map<ChildMsg, F>(&self, rx: &Receiver<ChildMsg>, f: F)
    where
        F: Fn(&ChildMsg) -> Option<Msg> + 'static,
    {
        rx.branch().forward_filter_map(&self.tx, f)
    }

    /// Subscribe to a receiver by forwarding messages from it using a map function.
    pub fn subscribe_map<ChildMsg, F>(&self, rx: &Receiver<ChildMsg>, f: F)
    where
        F: Fn(&ChildMsg) -> Msg + 'static,
    {
        rx.branch()
            .forward_filter_map(&self.tx, move |msg| Some(f(msg)))
    }

    /// Subscribe to a receiver by forwarding messages from it.
    pub fn subscribe(&self, rx: &Receiver<Msg>) {
        rx.branch().forward_map(&self.tx, |msg| msg.clone())
    }

    /// Send a one-time asynchronous message.
    pub fn send_async<F>(&self, f: F)
    where
        F: Future<Output = Msg> + 'static,
    {
        self.tx.send_async(f);
    }
}

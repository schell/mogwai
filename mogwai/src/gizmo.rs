use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Node;
pub use web_sys::{Element, Event, EventTarget, HtmlInputElement};

use crate::{
    prelude::{txrx, Component, IsDomNode, Receiver, Subscriber, Transmitter, ViewBuilder},
    utils,
};


/// A widget and all of its pieces.
pub struct Gizmo<T: Component> {
    pub trns: Transmitter<T::ModelMsg>,
    pub recv: Receiver<T::ViewMsg>,
    pub state: Rc<RefCell<T>>,
}


impl<T> Gizmo<T>
where
    T: Component + 'static,
    T::ViewMsg: Clone,
    T::DomNode: JsCast + AsRef<Node> + Clone,
{
    /// Create a new [`Gizmo`] from an initial state using
    /// a view and the [`Transmitter`] + [`Receiver`] used to
    /// create that view.
    pub fn from_parts(
        init: T,
        tx_in: Transmitter<T::ModelMsg>,
        rx_out: Receiver<T::ViewMsg>,
    ) -> Self {
        let state = Rc::new(RefCell::new(init));
        let tx_out = rx_out.new_trns();
        let rx_in = tx_in.spawn_recv();
        let subscriber = Subscriber::new(&tx_in);

        let (tx_view, rx_view) = txrx();
        rx_in.respond_shared(state.clone(), move |t: &mut T, msg: &T::ModelMsg| {
            T::update(t, msg, &tx_view, &subscriber);
        });

        rx_view.respond(move |msg: &T::ViewMsg| {
            let tx_out = tx_out.clone();
            let msg = msg.clone();
            utils::set_immediate(move || tx_out.send(&msg));
        });

        Gizmo {
            trns: tx_in,
            recv: rx_out,
            state,
        }
    }

    /// Create a new [`Gizmo`] from a stateful [`Component`].
    /// This will create a 'fresh' view.
    pub fn new(init: T) -> Gizmo<T> {
        let tx_in = Transmitter::new();
        let rx_out = Receiver::new();

        Gizmo::from_parts(init, tx_in, rx_out)
    }

    /// Use the Gizmo to spawn a [`ViewBuilder<T::DomNode>`].
    /// This allows you to send the builder (or subsequent view) somewhere else while still
    /// maintaining the ability to update the view from afar.
    pub fn view_builder(&self) -> ViewBuilder<T::DomNode> {
        self.state.as_ref().borrow().view(&self.trns, &self.recv)
    }

    /// Update the component with the given message.
    /// This how a parent component communicates down to its child components.
    pub fn send(&self, msg: &T::ModelMsg) {
        self.trns.send(msg);
    }

    /// Access the underlying state.
    pub fn with_state<F, N>(&self, f: F) -> N
    where
        F: Fn(&T) -> N,
    {
        let t = self.state.borrow();
        f(&t)
    }

    /// Set this gizmo's state.
    ///
    /// This silently updates the state and doesn't trigger any messages
    /// and does *not* update the view.
    pub fn set_state(&mut self, t: T) {
        *self.state.borrow_mut() = t;
    }

    /// Borrow a reference to the inner state.
    pub fn state_ref(&self) -> Ref<T> {
        self.state.borrow()
    }
}


impl<T: Component> From<T> for Gizmo<T> {
    fn from(component: T) -> Gizmo<T> {
        Gizmo::new(component)
    }
}


/// The type of function that uses a txrx pair and returns a View.
pub type BuilderFn<T, D> = dyn Fn(&Transmitter<T>, &Receiver<T>) -> ViewBuilder<D>;


/// A simple component made from a [BuilderFn].
///
/// Any function that takes a transmitter and receiver of the same type and
/// returns a [View] can be made into a component that holds no internal
/// state. It forwards all of its incoming messages to its view.
///
/// ```rust,no_run
/// extern crate mogwai;
/// use mogwai::prelude::*;
///
/// let component = Gizmo::from(SimpleComponent::new(
///     |tx: &Transmitter<()>, rx: &Receiver<()>| -> ViewBuilder<HtmlElement> {
///         builder!{
///             <button style="pointer" on:click=tx.contra_map(|_| ())>
///                 {("Click me", rx.branch_map(|()| "Clicked!".to_string()))}
///             </button>
///         }
///     }
/// ));
/// ```
pub struct SimpleComponent<T, D: IsDomNode>(Box<BuilderFn<T, D>>);


impl<T, D: IsDomNode> SimpleComponent<T, D> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Transmitter<T>, &Receiver<T>) -> ViewBuilder<D> + 'static,
    {
        SimpleComponent(Box::new(f))
    }
}


impl<T, D> Component for SimpleComponent<T, D>
where
    T: Clone + 'static,
    D: JsCast + AsRef<Node> + Clone + 'static,
{
    type ModelMsg = T;
    type ViewMsg = T;
    type DomNode = D;

    fn update(&mut self, msg: &T, tx_view: &Transmitter<T>, _sub: &Subscriber<T>) {
        tx_view.send(msg);
    }

    fn view(&self, tx: &Transmitter<T>, rx: &Receiver<T>) -> ViewBuilder<D> {
        self.0(tx, rx)
    }
}

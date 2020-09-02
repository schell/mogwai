use std::{cell::RefCell, rc::Rc};
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Node;
pub use web_sys::{Element, Event, EventTarget, HtmlInputElement};

use crate::prelude::{txrx, Component, Receiver, Subscriber, Transmitter, View, ViewBuilder};
use crate::utils;


/// A widget and all of its pieces.
pub struct Gizmo<T: Component> {
    pub trns: Transmitter<T::ModelMsg>,
    pub recv: Receiver<T::ViewMsg>,
    pub view: View<T::DomNode>,
    pub state: Rc<RefCell<T>>,
}


impl<T> Gizmo<T>
where
    T: Component + 'static,
    T::ViewMsg: Clone,
    T::DomNode: AsRef<Node> + Clone,
{
    /// Create a new [`Gizmo`] from an initial state using
    /// a view and the [`Transmitter`] + [`Receiver`] used to
    /// create that view.
    pub fn from_parts(
        init: T,
        tx_in: Transmitter<T::ModelMsg>,
        rx_out: Receiver<T::ViewMsg>,
        view: View<T::DomNode>,
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
            view,
            state,
        }
    }

    /// Create a new [`Gizmo`] from a stateful [`Component`].
    /// This will create a 'fresh' view.
    pub fn new(init: T) -> Gizmo<T> {
        let tx_in = Transmitter::new();
        let rx_out = Receiver::new();
        let view_builder = init.view(&tx_in, &rx_out);
        let view = view_builder.fresh_view();

        Gizmo::from_parts(init, tx_in, rx_out, view)
    }


    /// Hydrates a new [`Gizmo`] from a stateful [`Component`].
    /// If the view cannot be hydrated an error is returned.
    pub fn hydrate(init: T) -> Result<Gizmo<T>, crate::view::hydration::Error> {
        let tx_in = Transmitter::new();
        let rx_out = Receiver::new();
        let view_builder = init.view(&tx_in, &rx_out);
        let view = view_builder.hydrate_view()?;

        Ok(Gizmo::from_parts(init, tx_in, rx_out, view))
    }


    /// Hydrates a new [`Gizmo`] from a stateful [`Component`].
    /// If the view cannot be hydrated then a fresh one will be created.
    pub fn hydrate_or_fresh(init: T) -> Gizmo<T> {
        let tx_in = Transmitter::new();
        let rx_out = Receiver::new();
        let view_builder = init.view(&tx_in, &rx_out);
        let view = view_builder.hydrate_or_else_fresh_view();

        Gizmo::from_parts(init, tx_in, rx_out, view)
    }

    /// A reference to the browser's DomNode.
    ///
    /// # Panics
    /// Only works in the browser. Panics outside of wasm32.
    pub fn dom_ref(&self) -> &T::DomNode {
        if cfg!(target_arch = "wasm32") {
            return self.view.as_ref().unchecked_ref::<T::DomNode>();
        }
        panic!("Gizmo::dom_ref is only available on wasm32")
    }

    pub fn view_ref(&self) -> &View<T::DomNode> {
        &self.view
    }

    /// Send model messages into this component from a `Receiver<T::ModelMsg>`.
    /// This is helpful for sending messages to this component from
    /// a parent component.
    pub fn rx_from(self, rx: Receiver<T::ModelMsg>) -> Gizmo<T> {
        rx.forward_map(&self.trns, |msg| msg.clone());
        self
    }

    /// Send view messages from this component into a `Transmitter<T::ViewMsg>`.
    /// This is helpful for sending messages to this component from
    /// a parent component.
    pub fn tx_into(self, tx: &Transmitter<T::ViewMsg>) -> Gizmo<T> {
        self.recv.branch().forward_map(&tx, |msg| msg.clone());
        self
    }

    /// Run this component forever, handing ownership over to the browser window.
    ///
    /// # Panics
    /// Only works in the browser. Panics on compilation targets that are not
    /// wasm32.
    pub fn run(self) -> Result<(), JsValue> {
        if cfg!(target_arch = "wasm32") {
            return self.view.run();
        }
        panic!("Gizmo::run is only available on wasm32")
    }

    /// Update the component with the given message.
    /// This how a parent component communicates down to its child components.
    pub fn update(&self, msg: &T::ModelMsg) {
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
pub struct SimpleComponent<T, D: JsCast>(Box<BuilderFn<T, D>>);


impl<T, D: JsCast> SimpleComponent<T, D> {
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

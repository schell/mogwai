//! Build trees of widgets using two-way message passing.
use std::{convert::TryFrom, pin::Pin};

use futures::stream::StreamExt;

use crate::{
    builder::ViewBuilder,
    channel::broadcast,
    target::{Sendable, Spawnable},
    view::View,
};

/// A component is a [`ViewBuilder`] and its logic (a [`Spawnable`] future).
pub struct Component<T> {
    /// View builder.
    pub builder: ViewBuilder<T>,
    /// Spawnable async widget logic.
    pub logic: Pin<Box<dyn Spawnable>>,
}

/// A `Component` can be created from a [`ViewBuilder`]. The resulting
/// `Component` will have a noop logic future (a noop update loop).
impl<T> From<ViewBuilder<T>> for Component<T> {
    fn from(builder: ViewBuilder<T>) -> Self {
        Component {
            builder,
            logic: Box::pin(async {}),
        }
    }
}

/// A [`ViewBuilder`] can be created from a `Component`.
///
/// The `Component`'s logic will be spawned as a post-build operation.
impl<T: Sendable> From<Component<T>> for ViewBuilder<T> {
    fn from(c: Component<T>) -> Self {
        let Component { builder, logic } = c;
        builder.with_post_build(|_| crate::spawn(logic))
    }
}

impl<T> Component<T>
where
    View<T>: TryFrom<ViewBuilder<T>>,
{
    /// Add a logic future to this component.
    pub fn with_logic(mut self, f: impl Spawnable) -> Self {
        self.logic = Box::pin(f);
        self
    }

    /// Attempts to build a [`View`] from the component's builder and
    /// spawns the logic into the async runtime.
    pub fn build(self) -> Result<View<T>, <View<T> as TryFrom<ViewBuilder<T>>>::Error> {
        let view: View<T> = View::try_from(self.builder)?;
        crate::spawn(self.logic);
        Ok(view)
    }
}

/// A component that facilitates an Elm-inspired type of composure.
///
/// ## Types
/// * `T` - Inner view type, eg [`crate::view::Dom`]
/// * `S` - Logic state
/// * `LogicMsg` - Message type sent to the logic
/// * `ViewMsg` - Message type sent to the view
pub struct ElmComponent<T, S, LogicMsg, ViewMsg> {
    /// Initial state.
    pub state: S,

    /// Function for creating a [`ViewBuilder`].
    pub builder_fn: Box<
        dyn FnOnce(&S, broadcast::Sender<LogicMsg>, broadcast::Receiver<ViewMsg>) -> ViewBuilder<T>,
    >,

    /// Function that creates the Spawnable async widget logic.
    pub logic_fn: Box<
        dyn FnOnce(
            S,
            broadcast::Receiver<LogicMsg>,
            broadcast::Sender<ViewMsg>,
        ) -> Pin<Box<dyn Spawnable>>,
    >,
}

impl<T: Sendable, S: Default, LogicMsg, ViewMsg> Default for ElmComponent<T, S, LogicMsg, ViewMsg> {
    fn default() -> Self {
        Self {
            state: Default::default(),
            builder_fn: Box::new(|_, _, _| todo!("forgot to give ElmComponent a builder function")),
            logic_fn: Box::new(|_, _, _| Box::pin(async move {})),
        }
    }
}

impl<T: Sendable, S, LogicMsg, ViewMsg> From<S> for ElmComponent<T, S, LogicMsg, ViewMsg> {
    fn from(state: S) -> Self {
        Self {
            state,
            builder_fn: Box::new(|_, _, _| todo!("forgot to give ElmComponent a builder function")),
            logic_fn: Box::new(|_, _, _| Box::pin(async move {})),
        }
    }
}

impl<T, S, LogicMsg, ViewMsg> From<ElmComponent<T, S, LogicMsg, ViewMsg>> for Component<T>
where
    View<T>: TryFrom<ViewBuilder<T>>,
{
    fn from(
        ElmComponent {
            state,
            builder_fn,
            logic_fn,
        }: ElmComponent<T, S, LogicMsg, ViewMsg>,
    ) -> Self {
        let (tx_logic, rx_logic) = broadcast::bounded(1);
        let (tx_view, rx_view) = broadcast::bounded(1);
        let builder = builder_fn(&state, tx_logic, rx_view);
        let logic = logic_fn(state, rx_logic, tx_view);

        Component::from(builder).with_logic(logic)
    }
}

impl<T, S: Sendable, LogicMsg: Clone + Sendable, ViewMsg: Clone + Sendable>
    ElmComponent<T, S, LogicMsg, ViewMsg>
{
    /// Set the builder function.
    pub fn with_builder_fn(
        mut self,
        f: impl FnOnce(&S, broadcast::Sender<LogicMsg>, broadcast::Receiver<ViewMsg>) -> ViewBuilder<T>
            + 'static,
    ) -> Self {
        self.builder_fn = Box::new(f);
        self
    }

    /// Set the logic function
    pub fn with_logic_fn<F, Fut>(mut self, f: F) -> Self
    where
        Fut: Spawnable,
        F: FnOnce(S, broadcast::Receiver<LogicMsg>, broadcast::Sender<ViewMsg>) -> Fut + Sendable,
    {
        self.logic_fn = Box::new(move |s, rx, tx| Box::pin(async move { f(s, rx, tx).await }));
        self
    }

    /// Use an update function as the body of the logic loop.
    ///
    /// This function will be run on each message that the logic loop receives.
    pub fn with_update<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut S, LogicMsg, broadcast::Sender<ViewMsg>) + Sendable,
    {
        self.logic_fn = Box::new(move |mut s, mut rx, tx| {
            Box::pin(async move {
                loop {
                    match rx.next().await {
                        Some(msg) => f(&mut s, msg, tx.clone()),
                        None => break,
                    }
                }
            })
        });
        self
    }
}

/// A convenience trait for defining components that have distinct logic and view messages.
///
/// See the [module level documentation](super::component) for more details.
pub trait IsElmComponent
where
    Self: Sized + Sendable,
    Self::LogicMsg: Sendable + Clone,
    Self::ViewMsg: Sendable + Clone,
    Self::ViewNode: Sendable + Clone,
    View<Self::ViewNode>: TryFrom<ViewBuilder<Self::ViewNode>>,
{
    /// Message type used to drive component state updates.
    type LogicMsg;

    /// Message type used to drive view DOM patching.
    type ViewMsg;

    /// The `T` type in [`ViewBuilder<T>`], eg `mogwai::view::Dom`.
    type ViewNode;

    /// Update this component in response to any received logic messages.
    /// This is essentially one iteration in the component's logic loop.
    fn update(&mut self, msg: Self::LogicMsg, tx_view: broadcast::Sender<Self::ViewMsg>);

    /// Produce this component's view using a `mogwai::channel::broadcast::Sender` of model input messages
    /// and a `mogwai::channel::broadcast::Receiver` of view output messages.
    ///
    /// Model messages flow from the view into the update function. View messages
    /// flow from the update function to the view.
    fn view(
        &self,
        tx: broadcast::Sender<Self::LogicMsg>,
        rx: broadcast::Receiver<Self::ViewMsg>,
    ) -> ViewBuilder<Self::ViewNode>;

    /// Converts the type into a [`Component`].
    fn to_component(self) -> Component<Self::ViewNode> {
        Component::from(
            ElmComponent::from(self)
                .with_builder_fn(Self::view)
                .with_update(Self::update),
        )
    }
}

//! Build trees of widgets using two-way message passing.
use std::{convert::TryFrom, pin::Pin};

use crate::{builder::ViewBuilder, target::{Sendable, Spawnable}, view::View};

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
        let view: View<T> =
            View::try_from(self.builder)?;
        crate::spawn(self.logic);
        Ok(view)
    }
}

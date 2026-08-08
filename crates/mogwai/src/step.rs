//! Pull-based stepping traits for widget event loops.
//!
//! Mogwai widgets are driven by a pull-based event loop: a caller awaits the
//! widget's next event, reacts to it, then awaits again. This module
//! formalizes that convention as four traits so the compiler — not prose —
//! enforces the contract and so generic composition (e.g. racing N children)
//! becomes possible.
//!
//! ## When to use which trait
//!
//! | Trait | Receiver | Use when |
//! |------|----------|----------|
//! | [`Step`] | `&self` | `step` only awaits event listeners (interior mutability). Lets a parent race multiple children concurrently. |
//! | [`StepMut`] | `&mut self` | `step` mutates the widget's own fields or drives a mutable resource. Children cannot be raced concurrently. |
//! | [`StepWith<T>`] | `&self` | A container of `T`-typed children that races a per-child future (supplied by a closure) against its own event. Return type is a GAT `Output<Ev>`. |
//! | [`StepWithMut<T>`] | `&mut self` | Same, but with exclusive access to each child. Return type is a GAT `Output<Ev>`. |
//!
//! ## Object safety
//!
//! These traits use `impl Future` returns (RPITIT) and are therefore **not**
//! object-safe. If a `dyn Step` need ever arises, add a boxed companion trait
//! with a blanket bridge as a non-breaking addition.

use std::{future::Future, pin::Pin};

/// Pull-based event source — immutable borrow.
///
/// Implement this when `step` only awaits event listeners (which use interior
/// mutability). This lets a parent race multiple children's `step()` futures
/// concurrently without borrow conflicts.
///
/// ## Example
///
/// ```no_run
/// use mogwai::{prelude::*, step::Step};
///
/// struct Button<V: View> {
///     on_click: V::EventListener,
/// }
///
/// impl<V: View> Step for Button<V> {
///     type Output = V::Event;
///     fn step(&self) -> impl Future<Output = V::Event> {
///         self.on_click.next()
///     }
/// }
/// ```
pub trait Step {
    /// The event produced by a single step. Must be `'static` so any `Step`
    /// can be plugged into a [`StepWith`] closure.
    type Output: 'static;

    /// Advance the widget by one event. May resolve to `Self::Output` or
    /// never resolve (e.g. a presentational component with no events).
    fn step(&self) -> impl Future<Output = Self::Output>;
}

/// Pull-based event source — exclusive borrow.
///
/// Implement this when `step` mutates the widget's own fields or drives a
/// mutable resource it owns. A parent cannot race two `StepMut` children
/// concurrently; use [`Step`] instead when concurrent racing is needed.
///
/// ## Example
///
/// ```no_run
/// use mogwai::{prelude::*, step::StepMut};
///
/// struct Counter {
///     count: u32,
///     on_click: <mogwai::web::Web as View>::EventListener,
/// }
///
/// impl StepMut for Counter {
///     type Output = ();
///     fn step_mut(&mut self) -> impl Future<Output = ()> {
///         async move {
///             let _ev = self.on_click.next().await;
///             self.count += 1;
///         }
///     }
/// }
/// ```
pub trait StepMut {
    /// The event produced by a single step. Must be `'static` so any
    /// `StepMut` can be plugged into a [`StepWithMut`] closure.
    type Output: 'static;

    /// Advance the widget by one event, mutating internal state.
    fn step_mut(&mut self) -> impl Future<Output = Self::Output>;
}

/// A container of `T`-typed children that races a per-child future (supplied by
/// a closure) against its own event future — immutable borrow.
///
/// This generalizes the `List::step` / `ButtonGroup::step` / `TabList::step`
/// pattern: the container owns `N` children of type `T`, and the caller
/// decides how each child produces a future of type `Ev`.
///
/// The return type is a generic associated type (GAT) `Output<Ev>` so a
/// container can produce a different event type depending on the child event
/// `Ev` (e.g. an enum with `Tabs(Self::TabEvent)` and `Panes(Ev)` variants).
///
/// ## Example
///
/// ```no_run
/// use mogwai::{
///     prelude::*,
///     step::{Step, StepWith},
/// };
/// use std::pin::Pin;
///
/// struct List<V: View, T> {
///     items: Vec<T>,
///     // ...
/// #   _phantom: std::marker::PhantomData<V>,
/// }
///
/// impl<V: View, T> StepWith<T> for List<V, T> {
///     type Output<Ev: 'static> = ();
///     fn step_with<Ev>(
///         &self,
///         f: impl for<'a> FnMut(&'a T) -> Pin<Box<dyn Future<Output = Ev> + 'a>>,
///     ) -> impl Future<Output = Self::Output<Ev>>
///     where
///         Ev: 'static,
///     {
///         async move {
///             // race all children's futures...
/// #           std::future::pending::<()>().await
///         }
///     }
/// }
/// ```
pub trait StepWith<T> {
    /// The event produced by a single step, parameterized by the child event
    /// `Ev`. This is a generic associated type so containers can return a
    /// type that *contains* `Ev` (e.g. an enum with a `Panes(Ev)` variant).
    type Output<Ev: 'static>: 'static;

    /// Race the container's own event future against a future produced by
    /// `f` for each child. The first to resolve wins.
    ///
    /// The closure uses a higher-ranked trait bound (`for<'a>`) so the returned
    /// boxed future is allowed to borrow from the child reference for exactly
    /// as long as that borrow lives — e.g. `Box::pin(child.step())` where
    /// `step` borrows `&'a self`.
    fn step_with<Ev>(
        &self,
        f: impl for<'a> FnMut(&'a T) -> Pin<Box<dyn Future<Output = Ev> + 'a>>,
    ) -> impl Future<Output = Self::Output<Ev>>
    where
        Ev: 'static;
}

/// A container of `T`-typed children that races a per-child future (supplied by
/// a closure) against its own event future — mutable borrow.
///
/// This generalizes the `TabPanel::step_with` / `Table::step_with` pattern: the
/// container owns `N` children of type `T` mutably, and the caller decides how
/// each produces a future of type `Ev`.
///
/// The return type is a generic associated type (GAT) `Output<Ev>` so a
/// container can produce a different event type depending on the child event
/// `Ev` (e.g. `TabPanelEvent<V, T, Ev>` with `Tabs(...)` and `Panes(Ev)`
/// variants).
///
/// ## Example
///
/// ```no_run
/// use mogwai::{prelude::*, step::StepWithMut};
/// use std::pin::Pin;
///
/// struct TabPanel<V: View, P> {
///     panes: Vec<P>,
///     // ...
/// #   _phantom: std::marker::PhantomData<V>,
/// }
///
/// impl<V: View, P> StepWithMut<P> for TabPanel<V, P> {
///     type Output<Ev: 'static> = ();
///     fn step_with_mut<Ev>(
///         &mut self,
///         f: impl for<'a> FnMut(&'a mut P) -> Pin<Box<dyn Future<Output = Ev> + 'a>>,
///     ) -> impl Future<Output = Self::Output<Ev>>
///     where
///         Ev: 'static,
///     {
///         async move {
///             // race tab clicks against all pane futures...
/// #           std::future::pending::<()>().await
///         }
///     }
/// }
/// ```
pub trait StepWithMut<T> {
    /// The event produced by a single step, parameterized by the child event
    /// `Ev`. This is a generic associated type so containers can return a
    /// type that *contains* `Ev` (e.g. `TabPanelEvent<V, T, Ev>`).
    type Output<Ev: 'static>: 'static;

    /// Race the container's own event future against a future produced by
    /// `f` for each child (with mutable access). The first to resolve wins.
    ///
    /// The closure uses a higher-ranked trait bound (`for<'a>`) so the returned
    /// boxed future is allowed to borrow from the child reference for exactly
    /// as long as that borrow lives — e.g. `Box::pin(child.step_mut())` where
    /// `step_mut` borrows `&'a mut self`.
    fn step_with_mut<Ev>(
        &mut self,
        f: impl for<'a> FnMut(&'a mut T) -> Pin<Box<dyn Future<Output = Ev> + 'a>>,
    ) -> impl Future<Output = Self::Output<Ev>>
    where
        Ev: 'static;
}

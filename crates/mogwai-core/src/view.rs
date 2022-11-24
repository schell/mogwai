//! Trait supporting domain specific views.

/// An event target declaration.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum EventTargetType {
    /// This target is the view it is declared on.
    Myself,
    /// This target is the window.
    Window,
    /// This target is the document.
    Document,
}

/// An interface for a domain-specific view.
pub trait View
where
    Self: Sized + Clone + Unpin + 'static,
{
    /// The type of view events.
    type Event;

    ///// Spawn a future
    //fn spawn<T: Send + Sync + 'static>(f: impl Spawnable<T>);
}

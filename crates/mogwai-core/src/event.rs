//! View events as streams of values.
//!
//! Traits and types supporting named events.
use crate::target::Sinkable;

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

/// Trait for inner view types that support adding events.
///
/// Impliting `Eventable` supports `ViewBuilder::with_event` and the `on:{event}`
/// RSX attribute.
pub trait Eventable {
    /// Domain specific event type, eg `web_sys::Event`.
    type Event;

    /// Add an event sink to the view, sending each occurance into the sink.
    fn add_event_sink(
        &mut self,
        event_name: &str,
        target: EventTargetType,
        tx_event: impl Sinkable<Self::Event>,
    );
}

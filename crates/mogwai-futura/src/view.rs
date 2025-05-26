//! Traits for building cross-platform views.
use crate::Str;

pub trait ViewText {
    fn new(text: impl Into<Str>) -> Self;
    fn set_text(&self, text: impl Into<Str>);
}

pub trait ViewParent {
    type Node;

    fn append_child(&self, child: &impl ViewChild<Node = Self::Node>);
}

pub trait ViewChild {
    type Node;

    fn as_child(&self) -> Self::Node;
}

pub trait ViewEventListener {
    type Event;

    fn next(&self) -> impl Future<Output = Self::Event>;
}

pub trait ViewEventTarget {
    fn listen(&self, event_name: impl Into<Str>) -> impl ViewEventListener;
}

pub trait View {
    type Element<T>: ViewParent + ViewChild
    where
        T: ViewParent + ViewChild;
    type Text: ViewText + ViewChild;
    type EventListener: ViewEventListener;
}

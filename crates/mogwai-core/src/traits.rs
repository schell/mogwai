use std::{future::Future, pin::Pin};

use futures::{Stream, Sink};

use crate::channel::SinkError;

pub struct NoConstraint;
pub struct SendConstraint;
pub struct SendSyncConstraint;

type FuturePinBox<T> = Pin<Box<dyn Future<Output = T> + Unpin + 'static>>;
type FuturePinBoxSend<T> = Pin<Box<dyn Future<Output = T> + Unpin + Send + 'static>>;
type FuturePinBoxSendSync<T> = Pin<Box<dyn Future<Output = T> + Unpin + Send + Sync + 'static>>;

type StreamPinBox<T> = Pin<Box<dyn Stream<Item = T> + Unpin + 'static>>;
type StreamPinBoxSend<T> = Pin<Box<dyn Stream<Item = T> + Unpin + Send + 'static>>;
type StreamPinBoxSendSync<T> = Pin<Box<dyn Stream<Item = T> + Unpin + Send + Sync + 'static>>;

type SinkPinBox<T> = Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + 'static>>;
type SinkPinBoxSend<T> = Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + Send + 'static>>;
type SinkPinBoxSendSync<T> = Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + Send + Sync + 'static>>;

pub trait ConstraintType {
    type FutureType<T>: Future<Output = T>;
    type StreamType<T>: Stream<Item = T> + Unpin;
    type SinkType<T>: Sink<T, Error = SinkError>;
}

impl ConstraintType for NoConstraint {
    type FutureType<T> = FuturePinBox<T>;
    type StreamType<T> = StreamPinBox<T>;
    type SinkType<T> = SinkPinBox<T>;
}

impl ConstraintType for SendConstraint {
    type FutureType<T> = FuturePinBoxSend<T>;
    type StreamType<T> = StreamPinBoxSend<T>;
    type SinkType<T> = SinkPinBoxSend<T>;
}

impl ConstraintType for SendSyncConstraint {
    type FutureType<T> = FuturePinBoxSendSync<T>;
    type StreamType<T> = StreamPinBoxSendSync<T>;
    type SinkType<T> = SinkPinBoxSendSync<T>;
}

pub type ConstrainedFuture<T, C> = <C as ConstraintType>::FutureType<T>;
pub type ConstrainedStream<T, C> = <C as ConstraintType>::StreamType<T>;
pub type ConstrainedSink<T, C> = <C as ConstraintType>::SinkType<T>;

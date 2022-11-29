//! DOM building.
use std::{marker::PhantomData, pin::Pin};

use async_executor::{Executor, LocalExecutor};
use futures::Future;
pub use mogwai::{builder::*, view::{ConstrainedStream, ConstrainedFuture, View}};
use mogwai::{
    futures::StreamExt,
    patch::{HashPatch, ListPatch},
};

use crate::view::JsDom;

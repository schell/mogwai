//! Re-exports of the [futures_lite] crate, along with extensions and helper types.
use std::{
    sync::{Arc, Mutex, RwLock},
    task::Waker,
};

pub use futures_lite::future::*;

use crate::sink::{SendError, Sink, TrySendError};

/// A future view, which uses `Sink` to store the result of an
/// operation.
///
/// The `T` value is meant to be a smart pointer to a view. `T` is
/// sent into `Captured` via [`Sink::send`] or [`Sink::try_send`]. When it
/// is retrieved via `.await` a _clone_ of the `T` is the result.
pub struct Captured<T> {
    waker: Arc<Mutex<Option<Waker>>>,
    inner: Arc<RwLock<Option<T>>>,
}

impl<T: Clone> Clone for Captured<T> {
    fn clone(&self) -> Self {
        Self {
            waker: self.waker.clone(),
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for Captured<T> {
    fn default() -> Self {
        Self {
            waker: Arc::new(Mutex::new(None)),
            inner: Arc::new(RwLock::new(None)),
        }
    }
}

impl<T: Clone> Future for Captured<T> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let read = self.inner.read().unwrap();
        if let Some(t) = read.as_ref() {
            std::task::Poll::Ready(t.clone())
        } else {
            let waker = cx.waker().clone();
            *self.waker.lock().unwrap() = Some(waker);
            std::task::Poll::Pending
        }
    }
}

impl<T: Send + Sync + Clone> Sink<T> for Captured<T> {
    fn send(
        &self,
        item: T,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + '_>> {
        Box::pin(std::future::ready(
            self.try_send(item).map_err(|_| SendError::Full),
        ))
    }

    fn try_send(&self, item: T) -> Result<(), TrySendError> {
        // UNWRAP: if we can't get this write, we want the program to panic
        let mut write = self.inner.write().unwrap();
        if write.is_some() {
            Err(TrySendError::Full)
        } else {
            *write = Some(item);
            // UNWRAP: if we can't get this lock, we want the program to panic
            if let Some(waker) = self.waker.lock().unwrap().take() {
                waker.wake();
            }
            Ok(())
        }
    }
}

impl<T: Send + Sync + Clone> Captured<T> {
    /// Return a sink.
    pub fn sink(&self) -> impl Sink<T> {
        self.clone()
    }

    /// Gives the current value syncronously, if possible.
    pub fn current(&self) -> Option<T> {
        let lock = self.inner.read().ok()?;
        lock.as_ref().cloned()
    }

    /// Await and return a clone of the inner `T`.
    ///
    /// Alternatively you can simply use `.await`, consuming this `Captured`.
    pub async fn get(&self) -> T {
        self.clone().await
    }
}

pub struct JoinAll<Fut: Future>(Vec<(usize, Fut)>, Vec<Option<Fut::Output>>);

pub fn join_all<Fut: Future + Unpin>(futures: impl IntoIterator<Item = Fut>) -> JoinAll<Fut>
    where
    Fut::Output: Unpin
{

    let futures: Vec<(usize, Fut)> = futures.into_iter().enumerate().collect();
    let items: Vec<Option<Fut::Output>> = futures.iter().map(|_| None).collect();
    JoinAll(futures, items)
}

impl<Fut: Future + Unpin> Future for JoinAll<Fut>
where
    Fut::Output: Unpin,
{
    type Output = Vec<Fut::Output>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.0.is_empty() {
            let items = std::mem::take(&mut self.get_mut().1);
            std::task::Poll::Ready(items.into_iter().flat_map(|o| o).collect())
        } else {
            let join = self.get_mut();
            join.0.retain_mut(|(index, fut)| match fut.poll(cx) {
                std::task::Poll::Ready(output) => {
                    join.1[*index] = Some(output);
                    false
                }
                std::task::Poll::Pending => true,
            });
            std::task::Poll::Pending
        }
    }
}

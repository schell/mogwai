//! Types and extention traits for [`Stream`]s.
//!
//! Re-exports some of the futures crate, along with extensions and helper
//! types.
use std::{collections::VecDeque, pin::Pin};

pub use futures_lite::stream::*;

impl<T: ?Sized> StreamableExt for T where T: Stream {}

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedStreamLocal<'a, T> = Pin<Box<dyn Stream<Item = T> + Send + Sync + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type BoxedStreamLocal<'a, T> = Pin<Box<dyn Stream<Item = T> + 'a>>;

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedStream<T> = Pin<Box<dyn Stream<Item = T> + Send + Sync + 'static>>;
#[cfg(target_arch = "wasm32")]
pub type BoxedStream<T> = Pin<Box<dyn Stream<Item = T> + 'static>>;

#[cfg(not(target_arch = "wasm32"))]
pub trait StreamableExt {
    fn pinned_local<'a>(self) -> BoxedStreamLocal<'a, Self::Item>
    where
        Self: Sized + Send + Sync + Stream + 'a,
    {
        Box::pin(self)
    }

    fn pinned(self) -> BoxedStream<Self::Item>
    where
        Self: Sized + Send + Sync + Stream + 'static,
    {
        Box::pin(self)
    }
}

#[cfg(target_arch = "wasm32")]
pub trait StreamableExt {
    fn pinned_local<'a>(self) -> BoxedStreamLocal<'a, Self::Item>
    where
        Self: Sized + Stream + 'a,
    {
        Box::pin(self)
    }

    fn pinned(self) -> BoxedStream<Self::Item>
    where
        Self: Sized + Stream + 'static,
    {
        Box::pin(self)
    }
}

pub struct SelectAll<I, St>(Option<I>, VecDeque<St>);

impl<I: Iterator<Item = St>, St> SelectAll<I, St> {
    fn dequeue(&mut self) -> Option<St> {
        if self.0.is_some() {
            // UNWRAP: ok because we know this is Some
            let unpolled = self.0.as_mut().unwrap();
            let next = unpolled.next();
            if next.is_some() {
                return next;
            } else {
                self.0 = None;
            }
        }

        self.1.pop_front()
    }

    fn enqueue(&mut self, st: St) {
        self.1.push_back(st);
    }

    fn len(&self) -> usize {
        self.0.as_ref().map(|vs| vs.size_hint().0).unwrap_or_default() + self.1.len()
    }
}

pub fn select_all<I: IntoIterator<Item = St>, T: 'static, St: Stream<Item = T> + Send + Unpin + 'static>(
    streams: I,
) -> Option<SelectAll<I::IntoIter, St>> {
    let unpolled_streams = streams.into_iter();
    let (len, _) = unpolled_streams.size_hint();
    if len > 0 {
        let streams = VecDeque::with_capacity(unpolled_streams.size_hint().0);
        Some(SelectAll(Some(unpolled_streams), streams))
    } else {
        None
    }
}

impl<I: Iterator<Item = St> + Unpin, T: 'static, St: Stream<Item = T> + Send + Unpin + 'static> Stream for SelectAll<I, St> {
    type Item = T;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let len = self.len();
        if len == 0 {
            return std::task::Poll::Ready(None);
        }
        let many = self.get_mut();
        for _ in 0..len {
            let may_stream = many.dequeue();
            if let Some(mut st) = may_stream {
                match st.poll_next(cx) {
                    std::task::Poll::Ready(None) => {
                        // this stream will never yield again, don't enqueue it
                        // but check the others
                    }
                    std::task::Poll::Ready(Some(t)) => {
                        many.enqueue(st);
                        // this one is done
                        return std::task::Poll::Ready(Some(t));
                    }
                    std::task::Poll::Pending => {
                        many.enqueue(st);
                        // just go to the next one
                    }
                }
            }
        }
        std::task::Poll::Pending
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

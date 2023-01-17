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

pub struct SelectAll<St>(VecDeque<St>);

impl<St> std::fmt::Debug for SelectAll<St> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SelectAll").field(&self.0.len()).finish()
    }
}

pub fn select_all<T: 'static, St: Stream<Item = T> + Send + Unpin + 'static>(
    streams: impl IntoIterator<Item = St>,
) -> Option<SelectAll<St>> {
    let streams = streams.into_iter().collect::<VecDeque<_>>();
    if streams.len() > 0 {
        Some(SelectAll(streams))
    } else {
        None
    }
}

impl<T: 'static, St: Stream<Item = T> + Send + Unpin + 'static> Stream for SelectAll<St> {
    type Item = T;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut len = self.0.len();
        let many: &mut Self = self.get_mut();
        while len > 0 {
            len -= 1;
            if let Some(mut st) = many.0.pop_front() {
                match st.poll_next(cx) {
                    std::task::Poll::Ready(None) => {
                        if many.0.is_empty() {
                            // the streams are all gone and this one is empty,
                            // we'll never see another yield
                            return std::task::Poll::Ready(None);
                        }
                    }
                    std::task::Poll::Ready(Some(t)) => {
                        many.0.push_back(st);
                        return std::task::Poll::Ready(Some(t));
                    }
                    std::task::Poll::Pending => {
                        // just go to the next one
                        many.0.push_back(st);
                    }
                }
            }
        }
        std::task::Poll::Pending
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.0.len();
        (len, Some(len))
    }
}

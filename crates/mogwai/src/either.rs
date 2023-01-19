//! Sum type for either a "left" or "right" value.
//!
//! `Either` is similar to `Result` except that it doesn't represent errors.
//! As such, it has a different API.
use crate::{future::{Future, FutureExt}, stream::{Stream, StreamExt}};

/// Sum type for either a "left" or "right" value.
#[derive(Clone, Copy, Debug)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<T> Either<T, T> {
    pub fn into_inner(self) -> T {
        match self {
            Either::Left(t) => t,
            Either::Right(t) => t,
        }
    }
}

impl<A, B> Either<A, B> {
    pub fn left(self) -> Option<A> {
        match self {
            Either::Left(a) => Some(a),
            Either::Right(_) => None,
        }
    }

    pub fn right(self) -> Option<B> {
        match self {
            Either::Right(b) => Some(b),
            Either::Left(_) => None,
        }
    }

    pub fn as_left(&self) -> Option<&A> {
        match self {
            Either::Left(a) => Some(&a),
            Either::Right(_) => None,
        }
    }

    pub fn as_right(&self) -> Option<&B> {
        match self {
            Either::Left(_) => None,
            Either::Right(b) => Some(&b),
        }
    }

    pub fn as_ref(&self) -> Either<&A, &B> {
        match self {
            Either::Left(l) => Either::Left(&l),
            Either::Right(r) => Either::Right(&r),
        }
    }

    pub fn as_mut(&mut self) -> Either<&mut A, &mut B> {
        match self {
            Either::Left(l) => Either::Left(l),
            Either::Right(r) => Either::Right(r),
        }
    }

    pub fn bimap<F, G>(self, f: impl FnOnce(A) -> F, g: impl FnOnce(B) -> G) -> Either<F, G> {
        match self {
            Either::Left(l) => Either::Left(f(l)),
            Either::Right(r) => Either::Right(g(r)),
        }
    }

    pub fn either<T>(self, f: impl FnOnce(A) -> T, g: impl FnOnce(B) -> T) -> T {
        self.bimap(f, g).into_inner()
    }
}

impl<A: Stream + Unpin, B: Stream + Unpin> Stream for Either<A, B> {
    type Item = Either<A::Item, B::Item>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.get_mut().as_mut() {
            Either::Left(left) => left.poll_next(cx).map(|o| o.map(Either::Left)),
            Either::Right(right) => right.poll_next(cx).map(|o| o.map(Either::Right)),
        }
    }
}

impl<A: Future + Unpin, B: Future + Unpin> Future for Either<A, B> {
    type Output = Either<A::Output, B::Output>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.get_mut().as_mut() {
            Either::Left(left) => left.poll(cx).map(Either::Left),
            Either::Right(right) => right.poll(cx).map(Either::Right),
        }
    }
}

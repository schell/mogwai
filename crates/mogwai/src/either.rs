//! Sum type for either a "left" or "right" value.
//!
//! `Either` is similar to `Result` except that it doesn't represent errors.
//! As such, it has a different API.

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

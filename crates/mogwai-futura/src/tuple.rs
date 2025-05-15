//! Traits for preforming type-level operations on tuples.
//!
//! The traits are used for "smart" builders to organize return types.

pub trait Bundle {
    type Prefixed<T>;
    type Suffixed<T>;
    type Reduced;

    fn prefix<T>(self, element: T) -> Self::Prefixed<T>;
    fn suffix<T>(self, element: T) -> Self::Suffixed<T>;
    fn reduce(self) -> Self::Reduced;
}

impl Bundle for () {
    type Prefixed<T> = (T,);
    type Suffixed<T> = (T,);
    type Reduced = ();

    fn prefix<B>(self, element: B) -> Self::Prefixed<B> {
        (element,)
    }

    fn suffix<B>(self, element: B) -> Self::Suffixed<B> {
        (element,)
    }

    fn reduce(self) -> Self::Reduced {
        self
    }
}

impl<A> Bundle for (A,) {
    type Prefixed<B> = (B, A);
    type Suffixed<B> = (A, B);
    type Reduced = A;

    fn prefix<B>(self, element: B) -> Self::Prefixed<B> {
        (element, self.0)
    }

    fn suffix<B>(self, element: B) -> Self::Suffixed<B> {
        (self.0, element)
    }

    fn reduce(self) -> Self::Reduced {
        self.0
    }
}

macro_rules! suffix {
    ($($i:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($i),*> Bundle for ($($i),*) {
            type Prefixed<T> = (T, $($i),*);
            type Suffixed<T> = ($($i),*, T);
            type Reduced = Self;

            fn prefix<T>(self, element: T) -> Self::Prefixed<T> {
                let ($($i),*) = self;
                (element, $($i),*)
            }

            fn suffix<T>(self, element: T) -> Self::Suffixed<T> {
                let ($($i),*) = self;
                ($($i),*, element)
            }

            fn reduce(self) -> Self::Reduced {
                self
            }
        }
    };
}

suffix!(A, B);
suffix!(A, B, C);
suffix!(A, B, C, D);
suffix!(A, B, C, D, E);
suffix!(A, B, C, D, E, F);
suffix!(A, B, C, D, E, F, G);
suffix!(A, B, C, D, E, F, G, H);
suffix!(A, B, C, D, E, F, G, H, I);
suffix!(A, B, C, D, E, F, G, H, I, J);
suffix!(A, B, C, D, E, F, G, H, I, J, K);
suffix!(A, B, C, D, E, F, G, H, I, J, K, L);
suffix!(A, B, C, D, E, F, G, H, I, J, K, L, M);
suffix!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
suffix!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
suffix!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn sanity() {
        let bundle = ();
        let bundle: (f32,) = bundle.suffix(0.0);
        let bundle: (f32, u32) = bundle.suffix(0u32);
        let bundle: (f32, u32, char) = bundle.suffix('c');
        let _bundle: (&str, f32, u32, char) = bundle.prefix("blah");

        let bundle: (&str,) = ("hello",);
        let _bundle: &str = bundle.reduce();
    }
}

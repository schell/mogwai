//! Utilitites for working with futures.
//!
//! These are meant to be small additions to [`futures_lite`].
use std::{future::Future, pin::Pin};

use futures_lite::FutureExt;

pub trait MogwaiFutureExt
where
    Self: Sized + Future,
{
    /// Map the output of a future, producing a new future.
    fn map<T>(self, f: impl FnOnce(Self::Output) -> T) -> impl Future<Output = T> {
        async move {
            let output = self.await;
            f(output)
        }
    }
}

impl<T: Sized + Future> MogwaiFutureExt for T {}

/// Run all futures concurrently and return the output of the first future that resolves.
pub async fn race_all<T>(futs: impl IntoIterator<Item = impl Future<Output = T>>) -> T {
    let mut futures = futs
        .into_iter()
        .map(|f| Box::pin(f) as Pin<Box<dyn Future<Output = T>>>)
        .collect::<Vec<_>>();
    futures_lite::future::poll_fn(move |cx| {
        futures
            .iter_mut()
            .find_map(|fut| {
                let poll = fut.poll(cx);
                poll.is_ready().then_some(poll)
            })
            .unwrap_or(std::task::Poll::Pending)
    })
    .await
}

#[cfg(all(test, target_arch = "wasm32"))]
mod test {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn can_race() {
        async fn run(i: usize) -> usize {
            let _millis_waited = crate::time::wait_millis(i as u64).await;
            i
        }

        let i = race_all([run(10), run(100), run(200), run(400)]).await;
        assert_eq!(10, i);
    }
}

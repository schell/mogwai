use futures::StreamExt;

/// A wrapper around a domain-specific view.
pub struct View<T> {
    pub inner: T,
}

impl<T: Clone + 'static> View<T> {
    pub fn set_stream<S, F, A>(&self, mut setter: S, f:F)
    where
        S: futures::Stream<Item = A> + Unpin + 'static,
        F: Fn(&T, A) + 'static,
    {
        let t = self.inner.clone();
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                match setter.next().await {
                    Some(msg) => {
                        f(&t, msg);
                    }
                    None => {
                        break;
                    }
                }
            }
        });
    }
}

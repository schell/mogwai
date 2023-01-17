//! Wrapped views.
use std::{future::Future, pin::Pin, sync::Arc};

use anyhow::Context;
use async_executor::Executor;
use mogwai::{
    either::Either,
    patch::{ListPatch, ListPatchApply},
    view::{AnyEvent, AnyView, Downcast, Listener, Update, ViewBuilder},
};
pub use serde_json::Value;
pub use ssr::SsrDom;
use wasm_bindgen::JsCast;

pub use crate::event::JsDomEvent;

pub(crate) mod atomic;

pub mod js;
pub use js::JsDom;

use self::ssr::SsrDomEvent;

mod ssr;

pub(crate) struct FutureTask<T>(pub(crate) Pin<Box<dyn Future<Output = T> + Send>>);

#[derive(Clone)]
pub struct Dom(Either<JsDom, SsrDom>);

impl Downcast<Dom> for AnyView {
    fn downcast(self) -> anyhow::Result<Dom> {
        if cfg!(target = "wasm32") {
            let js: JsDom = self.downcast()?;
            Ok(Dom(Either::Left(js)))
        } else {
            let ssr: SsrDom = self.downcast()?;
            Ok(Dom(Either::Right(ssr)))
        }
    }
}

impl From<JsDom> for Dom {
    fn from(v: JsDom) -> Self {
        Dom(Either::Left(v))
    }
}

impl From<SsrDom> for Dom {
    fn from(v: SsrDom) -> Self {
        Dom(Either::Right(v))
    }
}

impl Dom {
    pub fn add_listener(dom: &Self, listener: Listener) -> anyhow::Result<()> {
        match &dom.0 {
            Either::Left(js) => js.add_listener(listener),
            Either::Right(ssr) => ssr.add_listener(listener),
        }
    }

    pub fn new(
        executor: Option<Arc<Executor<'static>>>,
        builder: ViewBuilder,
    ) -> anyhow::Result<Self> {
        Ok(Dom(if let Some(executor) = executor {
            Either::Right(ssr::build(&executor, builder)?)
        } else {
            Either::Left(js::build(builder, None)?)
        }))
    }

    pub fn executor(&self) -> Option<&Arc<Executor<'static>>> {
        self.as_either_ref().right().map(|ssr| &ssr.executor)
    }

    pub fn as_either_ref(&self) -> Either<&JsDom, &SsrDom> {
        match &self.0 {
            Either::Left(js) => Either::Left(js),
            Either::Right(ssr) => Either::Right(ssr),
        }
    }

    pub fn as_either_mut(&mut self) -> Either<&mut JsDom, &mut SsrDom> {
        match &mut self.0 {
            Either::Left(js) => Either::Left(js),
            Either::Right(ssr) => Either::Right(ssr),
        }
    }

    pub fn clone_as<T: JsCast + Clone>(&self) -> Option<T> {
        let js: &JsDom = self.as_either_ref().left()?;
        js.clone_as::<T>()
    }

    pub fn detach(&self) -> anyhow::Result<()> {
        let js: &JsDom = self
            .as_either_ref()
            .left()
            .context("cannot detach an SsrDom yet")?;
        js.detach();
        Ok(())
    }

    pub async fn html_string(&self) -> String {
        match self.as_either_ref() {
            Either::Left(js) => js.html_string().await,
            Either::Right(ssr) => ssr.html_string().await,
        }
    }

    pub async fn run_while<T: 'static>(
        &self,
        fut: impl Future<Output = T> + 'static,
    ) -> anyhow::Result<T> {
        match self.as_either_ref() {
            Either::Left(js) => js.run_while(fut).await,
            Either::Right(ssr) => ssr.run_while(fut).await,
        }
    }

    /// Run this element forever.
    ///
    /// ## Note
    /// * On WASM this hands ownership over to Javascript (in the browser
    ///   window)
    /// * On other targets this loops forever, running the server-side rendered
    ///   node's async tasks.
    pub fn run(self) -> anyhow::Result<()> {
        match self.0 {
            Either::Left(js) => js.run(),
            Either::Right(ssr) => loop {
                let _ = ssr.executor.try_tick();
            },
        }
    }

    pub fn update(&self, update: Update) -> anyhow::Result<()> {
        match update {
            Update::Child(patch) => {
                let patch: ListPatch<Dom> =
                    patch.try_map(|builder: ViewBuilder| -> anyhow::Result<Dom> {
                        Dom::new(self.executor().cloned(), builder)
                    })?;
                match self.clone().as_either_mut() {
                    Either::Left(js) => {
                        let patch: ListPatch<JsDom> = patch.try_map(|dom| {
                            anyhow::Ok(dom.as_either_ref().left().context("not js")?.clone())
                        })?;
                        let _ = js.list_patch_apply(patch);
                        Ok(())
                    }
                    Either::Right(ssr) => {
                        let patch: ListPatch<SsrDom> = patch.try_map(|dom| {
                            anyhow::Ok(dom.as_either_ref().right().context("not ssr")?.clone())
                        })?;
                        let _ = ssr.list_patch_apply(patch);
                        Ok(())
                    }
                }
            }
            update => match self.as_either_ref() {
                Either::Left(js) => js.update(update),
                Either::Right(ssr) => ssr.update(update),
            },
        }
    }
}

impl TryFrom<ViewBuilder> for Dom {
    type Error = anyhow::Error;

    fn try_from(builder: ViewBuilder) -> Result<Self, Self::Error> {
        let executor = if cfg!(target_arch = "wasm32") {
            None
        } else {
            Some(Arc::new(Executor::default()))
        };
        Dom::new(executor, builder)
    }
}

#[derive(Clone)]
pub struct DomEvent(Either<JsDomEvent, SsrDomEvent>);

impl Downcast<DomEvent> for AnyEvent {
    fn downcast(self) -> anyhow::Result<DomEvent> {
        if cfg!(target = "wasm32") {
            let js: JsDomEvent = self.downcast()?;
            Ok(DomEvent(Either::Left(js)))
        } else {
            let ssr: SsrDomEvent = self.downcast()?;
            Ok(DomEvent(Either::Right(ssr)))
        }
    }
}

impl DomEvent {
    pub fn as_either_ref(&self) -> Either<&JsDomEvent, &SsrDomEvent> {
        match &self.0 {
            Either::Left(js) => Either::Left(js),
            Either::Right(val) => Either::Right(val),
        }
    }
}

impl std::fmt::Debug for DomEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.as_either_ref() {
            Either::Left(js) => f.debug_tuple("DomEvent").field(js).finish(),
            Either::Right(val) => f.debug_tuple("DomEvent").field(val).finish(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn can_stream_my_select_all() {
        let usizes = futures_lite::stream::iter(vec![0usize, 1, 2, 3]);
        let floats = futures_lite::stream::iter(vec![0f32, 1.0, 2.0, 3.0]);
        let chars = futures_lite::stream::iter(vec!['a', 'b', 'c', 'd']);
        #[derive(Debug, PartialEq)]
        enum X {
            A(usize),
            B(f32),
            C(char),
        }
        let stream = select_all(vec![
            usizes.map(X::A).boxed(),
            floats.map(X::B).boxed(),
            chars.map(X::C).boxed(),
        ])
        .unwrap();
        let vals = futures_lite::future::block_on(stream.collect::<Vec<_>>());
        assert_eq!(
            vec![
                X::A(0),
                X::B(0.0),
                X::C('a'),
                X::A(1),
                X::B(1.0),
                X::C('b'),
                X::A(2),
                X::B(2.0),
                X::C('c'),
                X::A(3),
                X::B(3.0),
                X::C('d'),
            ],
            vals
        );
    }
}

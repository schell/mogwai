//! Wrapped views.
use std::{future::Future, pin::Pin, sync::Arc};

use anyhow::Context;
use async_executor::Executor;
use futures::{stream::SelectAll, stream, SinkExt, StreamExt};
use mogwai::{
    patch::{ListPatch, ListPatchApply},
    view::{exhaust, AnyView, Listener, Update, View, ViewBuilder, ViewIdentity},
    view::{MogwaiFuture, MogwaiSink, MogwaiStream, PostBuild},
};

pub use crate::event::JsDomEvent;

pub mod js;
pub use js::{JsDom, JsDomResources};

mod ssr;
pub use serde_json::Value;
pub use ssr::SsrDom;

pub use futures::future::Either;
pub use mogwai::futures::EitherExt;
use wasm_bindgen::JsCast;

/// Build the `ViewBuilder` in a way that can be used by the browser and server-side
/// and both.
pub(crate) fn build<V: View, R>(
    rez: R,
    builder: ViewBuilder,
    init: impl FnOnce(&R, ViewIdentity) -> anyhow::Result<V>,
    update_view: fn(&V, Update) -> anyhow::Result<()>,
    add_event: impl Fn(&V, Listener) -> anyhow::Result<()>,
) -> anyhow::Result<(V, Vec<Pin<Box<dyn Future<Output = ()> + Send>>>)> {
    let ViewBuilder {
        identity,
        updates,
        post_build_ops,
        listeners,
        tasks,
        view_sinks,
    } = builder;

    let updates = stream::select_all(updates);
    let (update_stream, initial_values) = exhaust(updates);
    let element: V = initialize_build(&rez, init, update_view, identity, initial_values)?; //
    finalize_build(
        element,
        update_stream,
        post_build_ops,
        listeners,
        tasks,
        view_sinks,
        add_event,
        update_view
    )
}

/// Initialize the DOM build by creating the element and applying any
/// updates that are ready and waiting.
pub(crate) fn initialize_build<V: View, R>(
    rez: &R,
    init: impl FnOnce(&R, ViewIdentity) -> anyhow::Result<V>,
    update_view: impl Fn(&V, Update) -> anyhow::Result<()> + Send + 'static,
    identity: ViewIdentity,
    initial_values: Vec<Update>,
) -> anyhow::Result<V> {
    let view = init(&rez, identity)?;
    for update in initial_values.into_iter() {
        update_view(&view, update)?;
    }
    Ok(view)
}

/// Finalize the DOM build by making the element reactive.
pub(crate) fn finalize_build<V: View>(
    element: V,
    mut update_stream: SelectAll<MogwaiStream<Update>>,
    post_build_ops: Vec<PostBuild>,
    listeners: Vec<Listener>,
    tasks: Vec<MogwaiFuture<()>>,
    view_sinks: Vec<MogwaiSink<AnyView>>,
    add_event: impl Fn(&V, Listener) -> anyhow::Result<()>,
    update_view: impl Fn(&V, Update) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<(V, Vec<Pin<Box<dyn Future<Output = ()> + Send>>>)> {
    let mut to_spawn: Vec<Pin<Box<dyn Future<Output = ()> + Send>>> = vec![];

    for listener in listeners.into_iter() {
        (add_event)(&element, listener)?;
    }

    let mut any_view = AnyView::new(element);
    for op in post_build_ops.into_iter() {
        (op)(&mut any_view)?;
    }
    let element = any_view.downcast::<V>()?;

    let node = element.clone();
    to_spawn.push(Box::pin(async move {
        while let Some(update) = update_stream.next().await {
            update_view(&node, update).unwrap();
        }
    }));

    for task in tasks.into_iter() {
        to_spawn.push(task);
    }
    let node: V = element.clone();
    println!("using {} node for sinks", std::any::type_name::<V>());
    to_spawn.push(Box::pin(async move {
        for mut sink in view_sinks.into_iter() {
            let any_view = AnyView::new(node.clone());
            println!("sinking {:?}", any_view);
            let _ = sink.send(any_view).await;
        }
    }));

    Ok((element, to_spawn))
}

#[derive(Clone)]
pub struct Dom(Either<JsDom, SsrDom>);

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
    fn init(
        rez: &Either<(), Arc<Executor<'static>>>,
        identity: ViewIdentity,
    ) -> anyhow::Result<Self> {
        Ok(match rez {
            Either::Left(()) => Dom::from(js::init(&(), identity)?),
            Either::Right(executor) => Dom::from(ssr::init(executor, identity)?),
        })
    }

    fn add_event(&self, listener: Listener) -> anyhow::Result<()> {
        match &self.0 {
            Either::Left(js) => js::add_event(js, listener),
            Either::Right(ssr) => ssr::add_event(ssr, listener),
        }
    }

    pub fn new(
        executor: Option<Arc<Executor<'static>>>,
        builder: ViewBuilder,
    ) -> anyhow::Result<Self> {
        let (dom, to_spawn) = build(
            executor
                .clone()
                .map(Either::Right)
                .unwrap_or_else(|| Either::Left(())),
            builder,
            Dom::init,
            Dom::update,
            Dom::add_event,
        )?;
        if let Some(executor) = executor {
            for task in to_spawn.into_iter() {
                executor.spawn(task).detach();
            }
        } else {
            for task in to_spawn.into_iter() {
                wasm_bindgen_futures::spawn_local(task);
            }
        }
        Ok(dom)
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
    /// * On WASM this hands ownership over to Javascript (in the browser window)
    /// * On other targets this loops forever, running the server-side rendered node's
    ///   async tasks.
    pub fn run(self) -> anyhow::Result<()> {
        match self.0 {
            Either::Left(js) => js.run(),
            Either::Right(ssr) => loop {
                let _ = ssr.executor.try_tick();
            },
        }
    }

    fn update(&self, update: Update) -> anyhow::Result<()> {
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
                Either::Left(js) => js::update_js_dom(js, update),
                Either::Right(ssr) => ssr::update_ssr_dom(ssr, update),
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
pub struct DomEvent(Either<JsDomEvent, Value>);

impl DomEvent {
    pub fn as_either_ref(&self) -> Either<&JsDomEvent, &Value> {
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

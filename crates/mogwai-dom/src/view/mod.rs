//! Wrapped views.
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{self, AtomicUsize},
        Arc,
    },
};

use anyhow::Context;
use async_executor::Executor;
use futures::{stream, stream::SelectAll, SinkExt, StreamExt};
use mogwai::{
    patch::{ListPatch, ListPatchApply},
    view::{exhaust, AnyView, Listener, Update, View, ViewBuilder, ViewIdentity},
    view::{MogwaiFuture, MogwaiSink, MogwaiStream, PostBuild},
};

pub use crate::event::JsDomEvent;

pub mod js;
pub use js::JsDom;

mod ssr;
pub use serde_json::Value;
pub use ssr::SsrDom;

pub use futures::future::Either;
pub use mogwai::futures::EitherExt;
use wasm_bindgen::JsCast;

static NODE_ID: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct FutureTask<T> {
    pub(crate) name: String,
    pub(crate) fut: Pin<Box<dyn Future<Output = T> + Send>>,
}

/// Build the `ViewBuilder` in a way that can be used by the browser and server-side
/// and both.
pub(crate) fn build<V: View, R>(
    rez: R,
    builder: ViewBuilder,
    init: impl FnOnce(&R, &str, usize, ViewIdentity) -> anyhow::Result<V>,
    update_view: fn(&V, Update) -> anyhow::Result<()>,
    add_event: impl Fn(&str, usize, &V, Listener) -> anyhow::Result<FutureTask<()>>,
) -> anyhow::Result<(V, Vec<FutureTask<()>>)> {
    let ViewBuilder {
        identity,
        updates,
        post_build_ops,
        listeners,
        tasks,
        view_sinks,
    } = builder;

    let id_string = match &identity {
        ViewIdentity::Leaf(text) => format!("\"{}\"", text),
        ViewIdentity::Branch(tag) => tag.clone(),
        ViewIdentity::NamespacedBranch(tag, _) => tag.clone(),
    };
    let node_id = NODE_ID.fetch_add(1, atomic::Ordering::Relaxed);
    let updates = stream::select_all(updates);
    let (update_stream, initial_values) = exhaust(updates);
    let element: V = initialize_build(
        &rez,
        &id_string,
        node_id,
        init,
        update_view,
        identity,
        initial_values,
    )?; //
    finalize_build(
        id_string,
        node_id,
        element,
        update_stream,
        post_build_ops,
        listeners,
        tasks,
        view_sinks,
        add_event,
        update_view,
    )
}

/// Initialize the DOM build by creating the element and applying any
/// updates that are ready and waiting.
pub(crate) fn initialize_build<V: View, R>(
    rez: &R,
    id_string: &str,
    node_id: usize,
    init: impl FnOnce(&R, &str, usize, ViewIdentity) -> anyhow::Result<V>,
    update_view: impl Fn(&V, Update) -> anyhow::Result<()> + Send + 'static,
    identity: ViewIdentity,
    initial_values: Vec<Update>,
) -> anyhow::Result<V> {
    let view = init(&rez, id_string, node_id, identity)?;
    for update in initial_values.into_iter() {
        update_view(&view, update)?;
    }
    Ok(view)
}

/// Finalize the DOM build by making the element reactive.
pub(crate) fn finalize_build<V: View>(
    id_string: String,
    node_id: usize,
    element: V,
    update_stream: SelectAll<MogwaiStream<Update>>,
    post_build_ops: Vec<PostBuild>,
    listeners: Vec<Listener>,
    tasks: Vec<MogwaiFuture<()>>,
    view_sinks: Vec<MogwaiSink<AnyView>>,
    add_event: impl Fn(&str, usize, &V, Listener) -> anyhow::Result<FutureTask<()>>,
    update_view: impl Fn(&V, Update) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<(V, Vec<FutureTask<()>>)> {
    let mut to_spawn: Vec<FutureTask<()>> = vec![];

    for listener in listeners.into_iter() {
        let fut_task = (add_event)(&id_string, node_id, &element, listener)?;
        to_spawn.push(fut_task);
    }

    let mut any_view = AnyView::new(element);
    for op in post_build_ops.into_iter() {
        (op)(&mut any_view)?;
    }
    let element = any_view.downcast::<V>()?;

    enum Upkeep {
        Timeout,
        Input(Update),
        End,
    }
    let node = element.clone();
    let id_string_clone = id_string.clone();
    to_spawn.push(FutureTask {
        name: format!("upkeep_{}_{}", id_string, node_id),
        fut: Box::pin(async move {
            let mut update_or_upkeep = stream::select_all(vec![
                update_stream
                    .map(Upkeep::Input)
                    .chain(stream::iter(std::iter::once(Upkeep::End)))
                    .boxed(),
                stream::unfold((), |()| async {
                    let _ = crate::core::time::wait_millis(1000).await;
                    Some((Upkeep::Timeout, ()))
                })
                .boxed(),
            ]);
            while let Some(upkeep) = update_or_upkeep.next().await {
                match upkeep {
                    Upkeep::Timeout => {
                        // do upkeep
                        log::trace!("upkeep on {} {}", node_id, id_string_clone);
                    }
                    Upkeep::Input(update) => {
                        update_view(&node, update).unwrap();
                    }
                    Upkeep::End => {
                        log::trace!("update task ending for {} {}", node_id, id_string_clone);
                        break;
                    }
                }
            }
        }),
    });

    for (i, task) in tasks.into_iter().enumerate() {
        to_spawn.push(FutureTask {
            name: format!("viewbuilder_task_{}_{}#{}", id_string, node_id, i),
            fut: task,
        });
    }
    let node: V = element.clone();
    println!("using {} node for sinks", std::any::type_name::<V>());
    to_spawn.push(FutureTask {
        name: format!("viewsink_{}_{}", id_string, node_id),
        fut: Box::pin(async move {
            for mut sink in view_sinks.into_iter() {
                let any_view = AnyView::new(node.clone());
                println!("sinking {:?}", any_view);
                let _ = sink.send(any_view).await;
            }
        }),
    });

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
        id_string: &str,
        node_id: usize,
        identity: ViewIdentity,
    ) -> anyhow::Result<Self> {
        Ok(match rez {
            Either::Left(()) => Dom::from(js::init(&(), id_string, node_id, identity)?),
            Either::Right(executor) => {
                Dom::from(ssr::init(executor, id_string, node_id, identity)?)
            }
        })
    }

    fn add_event(
        id_string: &str,
        node_id: usize,
        dom: &Self,
        listener: Listener,
    ) -> anyhow::Result<FutureTask<()>> {
        match &dom.0 {
            Either::Left(js) => js::add_event(id_string, node_id, js, listener),
            Either::Right(ssr) => ssr::add_event(id_string, node_id, ssr, listener),
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
            for fut_task in to_spawn.into_iter() {
                executor.spawn(fut_task.fut).detach();
            }
        } else {
            for fut_task in to_spawn.into_iter() {
                let js = dom.as_either_ref().left().context("impossible")?;
                let mut ts = js.tasks.try_write().context("can't write tasks")?;
                ts.push(js::spawn_local(&fut_task.name, fut_task.fut));
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

//! Wrapped views.
use anyhow::Context;
use futures::{stream, SinkExt, StreamExt};
use mogwai::view::{
    exhaust, AnyView, Listener, Update, View, ViewBuilder, ViewIdentity, ViewResources,
};
mod js_dom;

pub use js_dom::*;

mod ssr;
use serde_json::Value;
pub use ssr::*;

pub use futures::future::Either;
pub use mogwai::futures::EitherExt;
use wasm_bindgen::JsCast;

use crate::prelude::JsDomEvent;

fn build<V: View, R: ViewResources<V>>(
    rez: &mut R,
    builder: ViewBuilder,
    init: impl FnOnce(&mut R, ViewIdentity, Vec<Update>) -> anyhow::Result<V>,
    update_view: impl Fn(&V, Update) -> anyhow::Result<()> + Send + 'static,
    add_event: impl Fn(&V, Listener) -> anyhow::Result<()>,
) -> anyhow::Result<V> {
    let ViewBuilder {
        identity,
        updates,
        post_build_ops,
        listeners,
        tasks,
        view_sinks,
    } = builder;

    let updates = stream::select_all(updates);
    let (mut update_stream, initial_values) = exhaust(updates);
    let element = init(rez, identity, initial_values)?;

    for listener in listeners.into_iter() {
        (add_event)(&element, listener)?;
    }

    let mut any_view = AnyView::new(element);
    for op in post_build_ops.into_iter() {
        (op)(&mut any_view)?;
    }
    let element = any_view.downcast::<V>()?;

    let node = element.clone();
    rez.spawn(async move {
        while let Some(update) = update_stream.next().await {
            update_view(&node, update).unwrap();
        }
    });

    for task in tasks.into_iter() {
        rez.spawn(task);
    }
    let node = element.clone();
    rez.spawn(async move {
        for mut sink in view_sinks.into_iter() {
            let _ = sink.send(AnyView::new(node.clone())).await;
        }
    });

    Ok(element)
}

#[derive(Clone)]
pub struct Dom(mogwai::view::AnyView);

impl From<JsDom> for Dom {
    fn from(v: JsDom) -> Self {
        Dom(AnyView::new(v))
    }
}

impl From<SsrDom> for Dom {
    fn from(v: SsrDom) -> Self {
        Dom(AnyView::new(v))
    }
}

impl Dom {
    pub fn as_either_ref(&self) -> Either<&JsDom, &SsrDom> {
        if cfg!(target_arch = "wasm32") {
            // UNWRAP: safe because we only construct JsDom values
            // on wasm32
            let js: &JsDom = self.0.downcast_ref().unwrap();
            Either::Left(js)
        } else {
            // UNWRAP: safe because we only construct SsrDom values
            // on targets other than wasm32
            let ssr: &SsrDom = self.0.downcast_ref().unwrap();
            Either::Right(ssr)
        }
    }

    pub fn clone_as<T: JsCast + Clone>(&self) -> Option<T> {
        let js: &JsDom = self.as_either_ref().left()?;
        js.clone_as::<T>()
    }

    pub fn run(self) -> anyhow::Result<()> {
        if cfg!(target_arch = "wasm32") {
            let js_dom: JsDom = self.0.downcast()?;
            js_dom.run()
        } else {
            Ok(())
        }
    }

    pub fn detach(&self) -> anyhow::Result<()> {
        if cfg!(target_arch = "wasm32") {
            let js_dom: &JsDom = self.0.downcast_ref().context("not JsDom")?;
            js_dom.detach();
        }
        Ok(())
    }

    pub async fn html_string(&self) -> String {
        match self.as_either_ref() {
            Either::Left(js) => js.html_string().await,
            Either::Right(ssr) => ssr.html_string().await,
        }
    }
}

impl TryFrom<ViewBuilder> for Dom {
    type Error = anyhow::Error;

    fn try_from(value: ViewBuilder) -> Result<Self, Self::Error> {
        if cfg!(target_arch = "wasm32") {
            let js_dom: JsDom = value.build()?;
            Ok(Dom(AnyView::new(js_dom)))
        } else {
            let ssr_dom: SsrDom = value.build()?;
            Ok(Dom(AnyView::new(ssr_dom)))
        }
    }
}


#[derive(Clone)]
pub struct DomEvent(mogwai::view::AnyEvent);

impl DomEvent {
    pub fn as_either_ref(&self) -> Either<&JsDomEvent, &Value> {
        if cfg!(target_arch = "wasm32") {
            // UNWRAP: safe because we only construct JsDom values
            // on wasm32
            let js: &JsDomEvent = self.0.downcast_ref().unwrap();
            Either::Left(js)
        } else {
            // UNWRAP: safe because we only construct SsrDom values
            // on targets other than wasm32
            let ssr: &Value = self.0.downcast_ref().unwrap();
            Either::Right(ssr)
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

pub trait DomBuilder<T> {
    fn build(self) -> anyhow::Result<T>;
}

impl DomBuilder<JsDom> for mogwai::view::ViewBuilder {
    fn build(self) -> anyhow::Result<JsDom> {
        self.try_into()
    }
}

impl DomBuilder<SsrDom> for mogwai::view::ViewBuilder {
    fn build(self) -> anyhow::Result<SsrDom> {
        self.try_into()
    }
}

impl DomBuilder<Dom> for mogwai::view::ViewBuilder {
    fn build(self) -> anyhow::Result<Dom> {
        self.try_into()
    }
}

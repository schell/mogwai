//! Wrapped views.
use std::{future::Future, pin::Pin};

use crate::event::JsDomEvent;
use anyhow::Context;
pub use futures::future::Either;
use mogwai::prelude::*;
use serde_json::Value;

mod js_dom;
pub use js_dom::*;

mod ssr;
pub use ssr::*;

/// Adds helpful extensions to [`Either`].
pub trait EitherExt {
    /// The left item.
    type LeftItem;

    /// The right item.
    type RightItem;

    /// Return the left item, if possible.
    fn left(self) -> Option<Self::LeftItem>;

    /// Return the left item, if possible.
    fn right(self) -> Option<Self::RightItem>;
}

impl<A, B> EitherExt for Either<A, B> {
    type LeftItem = A;
    type RightItem = B;

    fn left(self) -> Option<Self::LeftItem> {
        match self {
            Either::Left(a) => Some(a),
            Either::Right(_) => None,
        }
    }

    fn right(self) -> Option<Self::RightItem> {
        match self {
            Either::Right(b) => Some(b),
            Either::Left(_) => None,
        }
    }
}

impl ViewResources<Dom> for Either<JsDomResources, SsrDomResources> {
    fn init(&self, identity: ViewIdentity) -> anyhow::Result<Dom> {
        match self {
            Either::Left(js) => Ok(Dom::Js(js.init(identity)?)),
            Either::Right(ss) => Ok(Dom::Ssr(ss.init(identity)?)),
        }
    }
}

/// Represents either `JsDom` or `SsrDom`.
///
/// Either can be picked to be constructed.
#[derive(Clone)]
pub enum Dom {
    Js(JsDom),
    Ssr(SsrDom),
}

impl From<JsDom> for Dom {
    fn from(js: JsDom) -> Self {
        Dom::Js(js)
    }
}

impl From<SsrDom> for Dom {
    fn from(ssr: SsrDom) -> Self {
        Dom::Ssr(ssr)
    }
}

impl Dom {
    pub fn visit_either<T>(&self, f: impl FnOnce(&JsDom) -> T, g: impl FnOnce(&SsrDom) -> T) -> T {
        match self {
            Dom::Js(js) => f(js),
            Dom::Ssr(ssr) => g(ssr),
        }
    }

    pub fn into_js(self) -> Option<JsDom> {
        match self {
            Dom::Js(js) => Some(js),
            Dom::Ssr(_) => None,
        }
    }

    pub fn into_ssr(self) -> Option<SsrDom> {
        match self {
            Dom::Js(_) => None,
            Dom::Ssr(ssr) => Some(ssr),
        }
    }

    pub async fn html_string(&self) -> String {
        match self {
            Dom::Js(js) => js.html_string().await,
            Dom::Ssr(ss) => ss.html_string().await,
        }
    }

    pub fn run_while<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> anyhow::Result<T> {
        match self {
            Dom::Js(js) => js.run_while(fut),
            Dom::Ssr(ssr) => ssr.run_while(fut),
        }
    }
}

/// Represents either `JsDomEvent` or `Value`.
pub enum DomEvent {
    Js(JsDomEvent),
    Ssr(Value),
}

impl View for Dom {
    /// The type of events supported by this view.
    type Event = DomEvent;

    /// The type of child views that can be nested inside this view.
    type Child = Dom;

    /// The type that holds domain specific resources used to
    /// construct views.
    type Resources = Either<JsDomResources, SsrDomResources>;

    /// Possibly asynchronous and scoped acquisition of resources.
    ///
    /// Used to build children before patching.
    fn with_acquired_resources<'a, T: Send + Sync + 'static>(
        &self,
        f: impl FnOnce(Self::Resources) -> anyhow::Result<T> + Send + Sync + 'a,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + Sync + 'a>> {
        match self {
            Dom::Js(js) => js.with_acquired_resources(|rez| f(Either::Left(rez))),
            Dom::Ssr(ssr) => ssr.with_acquired_resources(|rez| f(Either::Right(rez))),
        }
    }

    /// Set the text of this view.
    fn set_text(&self, s: &str) -> anyhow::Result<()> {
        self.visit_either(|js| js.set_text(s), |ssr| ssr.set_text(s))
    }

    /// Patch the attributes of this view.
    fn patch_attribs(&self, patch: HashPatch<String, String>) -> anyhow::Result<()> {
        match self {
            Dom::Js(js) => js.patch_attribs(patch),
            Dom::Ssr(ss) => ss.patch_attribs(patch),
        }
    }

    /// Patch the boolean attributes of this view.
    fn patch_bool_attribs(&self, patch: HashPatch<String, bool>) -> anyhow::Result<()> {
        match self {
            Dom::Js(js) => js.patch_bool_attribs(patch),
            Dom::Ssr(ssr) => ssr.patch_bool_attribs(patch),
        }
    }

    /// Patch the style attributes of this view.
    fn patch_styles(&self, patch: HashPatch<String, String>) -> anyhow::Result<()> {
        match self {
            Dom::Js(js) => js.patch_styles(patch),
            Dom::Ssr(ssr) => ssr.patch_styles(patch),
        }
    }

    /// Patch the nested children of this view.
    ///
    /// Returns a vector of the children removed.
    fn patch_children(&self, patch: ListPatch<Self::Child>) -> anyhow::Result<Vec<Self::Child>> {
        Ok(match self {
            Dom::Js(js) => js
                .patch_children(patch.try_map(|dom| dom.into_js().context("not js"))?)?
                .into_iter()
                .map(Dom::from)
                .collect(),
            Dom::Ssr(ssr) => ssr
                .patch_children(patch.try_map(|dom| dom.into_ssr().context("not ssr"))?)?
                .into_iter()
                .map(Dom::from)
                .collect(),
        })
    }

    /// Add an event to the element, document or window.
    ///
    /// When an event occurs it will be sent into the given sink.
    fn set_event(
        &self,
        type_is: EventTargetType,
        name: &str,
        sink: impl Sink<Self::Event, Error = SinkError> + Unpin + Send + Sync + 'static,
    ) -> anyhow::Result<()> {
        match self {
            Dom::Js(js) => js.set_event(type_is, name, sink.contra_map(DomEvent::Js)),
            Dom::Ssr(ss) => ss.set_event(type_is, name, sink.contra_map(DomEvent::Ssr)),
        }
    }

    fn spawn(&self, action: impl Future<Output = ()> + Send + 'static) {
        match self {
            Dom::Js(js) => js.spawn(action),
            Dom::Ssr(ss) => ss.spawn(action),
        }
    }
}

pub trait DomBuilder<T> {
    fn build(self) -> anyhow::Result<T>;
}

impl DomBuilder<JsDom> for mogwai::builder::ViewBuilder<JsDom> {
    fn build(self) -> anyhow::Result<JsDom> {
        self.try_into()
    }
}

impl DomBuilder<SsrDom> for mogwai::builder::ViewBuilder<SsrDom> {
    fn build(self) -> anyhow::Result<SsrDom> {
        self.try_into()
    }
}

/// When building `Dom` from a `ViewBuilder<Dom>`, the result will
/// be `Dom::Js(_)` on WASM and `Dom::Ssr(_)` otherwise.
impl DomBuilder<Dom> for ViewBuilder<Dom> {
    #[cfg(target_arch = "wasm32")]
    fn build(self) -> anyhow::Result<Dom> {
        Either::Left(JsDomResources).build(self)
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn build(self) -> anyhow::Result<Dom> {
        let rez = Either::Right(SsrDomResources::default());
        rez.build(self)
    }
}

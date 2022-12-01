//! Types and [`TryFrom`] instances that can 're-animate' views or portions of views from the DOM.
use futures::SinkExt;
use mogwai::{
    builder::{exhaust, ViewBuilder},
    dom::{spawn, view::Dom},
    futures::EitherExt,
    patch::{HashPatch, HashPatchApply, ListPatchApply},
};
// TODO: Standardize on anyhow instead of snafu
use snafu::{ensure, OptionExt, Snafu};
use std::collections::HashMap;
pub use std::{convert::TryFrom, ops::Deref};
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Node;
pub use web_sys::{Element, Event, EventTarget, HtmlElement};

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display(
        "Missing any hydration option for node '{}' - must be the child of a node or have an id",
        tag
    ))]
    NoHydrationOption { tag: String },

    #[snafu(display("Could not find an element with id '{}'", id))]
    MissingId { id: String },

    #[snafu(display("Child at index {} could not be found in node '{}' containing '{:?}'", index, node.node_name(), node.node_value()))]
    MissingChild { node: Node, index: u32 },

    #[snafu(display("Could not convert from '{}' to '{}' for value: {:#?}", from, to, node))]
    Conversion {
        from: String,
        to: String,
        node: JsValue,
    },

    #[snafu(display("View cannot be hydrated"))]
    ViewOnly,

    #[snafu(display("Hydration only available on WASM"))]
    WASMOnly { backtrace: snafu::Backtrace },

    #[snafu(display("Hydration failed: {}", msg))]
    Other { msg: String },
}

pub enum HydrationKey {
    Id(String),
    IndexedChildOf { node: Node, index: u32 },
}

impl HydrationKey {
    pub fn try_new(
        tag: String,
        attribs: Vec<HashPatch<String, String>>,
        may_parent: Option<(usize, &Node)>,
    ) -> Result<Self, Error> {
        let mut attributes = HashMap::new();
        for patch in attribs.into_iter() {
            let _ = attributes.hash_patch_apply(patch);
        }

        if let Some(id) = attributes.remove("id") {
            return Ok(HydrationKey::Id(id));
        }

        if let Some((index, parent)) = may_parent {
            return Ok(HydrationKey::IndexedChildOf {
                node: parent.clone(),
                index: index as u32,
            });
        }

        Err(Error::NoHydrationOption { tag })
    }

    pub fn hydrate(self) -> Result<Dom, Error> {
        snafu::ensure!(cfg!(target_arch = "wasm32"), WASMOnly);

        let el: Node = match self {
            HydrationKey::Id(id) => {
                let el = mogwai::dom::utils::document()
                    .clone_as::<web_sys::Document>()
                    .with_context(|| WASMOnly)?
                    .get_element_by_id(&id)
                    .with_context(|| MissingId { id })?;
                el.clone().dyn_into::<Node>().or_else(|_| {
                    Conversion {
                        from: "Element",
                        to: "Node",
                        node: el,
                    }
                    .fail()
                })?
            }
            HydrationKey::IndexedChildOf { node, index } => {
                let children = node.child_nodes();
                let mut non_empty_children = vec![];
                for i in 0..children.length() {
                    let child = children.get(i).with_context(|| MissingChild {
                        node: node.clone(),
                        index,
                    })?;
                    if child.node_type() == 3 {
                        // This is a text node
                        let has_text: bool = child
                            .node_value()
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| false);
                        if has_text {
                            non_empty_children.push(child);
                        }
                    } else {
                        non_empty_children.push(child);
                    }
                }
                let el = non_empty_children
                    .get(index as usize)
                    .with_context(|| MissingChild {
                        node: node.clone(),
                        index,
                    })?
                    .clone();
                el
            }
        };

        let dom = JsDom::try_from(JsValue::from(el));
        ensure!(dom.is_ok(), WASMOnly);

        Ok(dom.unwrap())
    }
}

pub struct Hydrator {
    inner: JsDom,
}

impl From<Hydrator> for JsDom {
    fn from(Hydrator { inner }: Hydrator) -> Self {
        inner
    }
}

impl TryFrom<ViewBuilder<JsDom>> for Hydrator {
    type Error = Error;

    fn try_from(builder: ViewBuilder<JsDom>) -> Result<Self, Self::Error> {
        Hydrator::try_hydrate(builder, None)
    }
}

impl Hydrator {
    /// Attempt to hydrate [`Dom`] from [`DecomposedViewBuilder<JsDom>`].
    fn try_hydrate(
        ViewBuilder {
            identity,
            texts,
            attribs,
            bool_attribs,
            styles,
            children,
            events,
            ops,
            view_sinks,
            tasks,
        }: ViewBuilder<JsDom>,
        may_parent: Option<(usize, &Node)>,
    ) -> Result<Hydrator, Error> {
        let construct_with = match identity {
            mogwai::prelude::ViewIdentity::Branch(s) => s,
            mogwai::prelude::ViewIdentity::NamespacedBranch(s, _) => s,
            mogwai::prelude::ViewIdentity::Leaf(s) => s,
        };

        let (text_stream, _texts) = exhaust(Box::pin(futures::stream::select_all(texts)));
        let (attrib_stream, attribs) = exhaust(Box::pin(futures::stream::select_all(attribs)));
        let (bool_attrib_stream, _bool_attribs) =
            exhaust(Box::pin(futures::stream::select_all(bool_attribs)));
        let (style_stream, _styles) = exhaust(Box::pin(futures::stream::select_all(styles)));
        let (child_stream, child_patches) =
            exhaust(Box::pin(futures::stream::select_all(children)));

        let key = HydrationKey::try_new(construct_with, attribs, may_parent)?;
        let mut dom = key.hydrate()?;

        mogwai::dom::builder::set_streaming_values(
            &dom,
            text_stream,
            attrib_stream,
            bool_attrib_stream,
            style_stream,
            child_stream,
        )
        .map_err(|msg| Error::Other { msg })?;

        let guard = dom.inner_read().left().with_context(|| WASMOnly)?;
        let node = guard.dyn_ref::<Node>().with_context(|| Conversion {
            from: format!("{:?}", guard.deref()),
            to: "Node".to_string(),
            node: guard.clone(),
        })?;

        let mut children = vec![];
        for patch in child_patches.into_iter() {
            let _ = children.list_patch_apply(patch);
        }

        for (bldr, i) in children.into_iter().zip(0..) {
            let _ = Hydrator::try_hydrate(bldr, Some((i, node)))?;
        }
        drop(guard);

        for (event_name, event_target, event_sink) in events.into_iter() {
            dom.set_event(event_target, &event_name, event_sink);
        }

        for op in ops.into_iter() {
            (op)(&mut dom);
        }

        for mut sink in view_sinks.into_iter() {
            let view = dom.clone();
            spawn(async move {
                let _ = sink.send(view).await;
            });
        }

        for task in tasks.into_iter() {
            spawn(task);
        }

        Ok(Hydrator { inner: dom })
    }
}

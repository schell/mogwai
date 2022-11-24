//! Wrapped views.
use async_executor::Executor;
pub use futures::future::Either;
use futures::SinkExt;
use mogwai::{
    builder::ViewBuilder,
    patch::ListPatch,
    traits::{ConstrainedFuture, ConstraintType, SendConstraint, SendSyncConstraint, NoConstraint},
};
use std::{
    future::Future,
    ops::{Bound, RangeBounds},
    pin::Pin,
};

mod dom;
pub use dom::*;

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

fn build<'a, C: ConstraintType>(
    spawn: impl Fn(ConstrainedFuture<(), C>),
    builder: ViewBuilder<Dom, C>,
) -> anyhow::Result<Dom> {
    let ViewBuilder {
        identity,
        texts,
        attribs,
        bool_attribs,
        styles,
        ops,
        children,
        events,
        view_sinks,
        tasks,
    } = builder;

    let mut element = match identity {
        mogwai::builder::ViewIdentity::Branch(tag) => Dom::element(&tag, None).unwrap(),
        mogwai::builder::ViewIdentity::NamespacedBranch(tag, ns) => {
            Dom::element(&tag, Some(&ns)).unwrap()
        }
        mogwai::builder::ViewIdentity::Leaf(text) => Dom::text(&text).unwrap(),
    };

    use mogwai::builder::exhaust;
    let (text_stream, texts) = exhaust(Box::pin(futures::stream::select_all(texts)));
    let (attrib_stream, attribs) = exhaust(Box::pin(futures::stream::select_all(attribs)));
    let (bool_attrib_stream, bool_attribs) =
        exhaust(Box::pin(futures::stream::select_all(bool_attribs)));
    let (style_stream, styles) = exhaust(Box::pin(futures::stream::select_all(styles)));
    let (child_stream, children) = exhaust(Box::pin(futures::stream::select_all(children)));

    crate::builder::set_initial_values(
        &element,
        texts.into_iter(),
        attribs.into_iter(),
        bool_attribs.into_iter(),
        styles.into_iter(),
        children.into_iter(),
    )?;

    crate::builder::set_streaming_values(
        spawn,
        &element,
        text_stream,
        attrib_stream,
        bool_attrib_stream,
        style_stream,
        child_stream,
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    for (event_name, event_target, event_sink) in events.into_iter() {
        element.set_event(event_target, &event_name, event_sink);
    }

    for op in ops.into_iter() {
        (op)(&mut element);
    }

    for mut sink in view_sinks.into_iter() {
        let view = element.clone();
        spawn(Box::pin(async move {
            // Try to send the dom but don't panic because
            // the recv may have been dropped already, and that's ok.
            let _ = sink.send(view).await;
        }));
    }

    for task in tasks.into_iter() {
        spawn(task);
    }

    Ok(element)
}

/// An extension trait that constructs `Dom` nodes.
pub trait DomBuilderExt<C: ConstraintType> {
    fn build(&self, builder: ViewBuilder<Dom, C>) -> anyhow::Result<Dom>;
}

impl<'a> DomBuilderExt<SendConstraint> for Executor<'a> {
    fn build(&self, builder: ViewBuilder<Dom, SendConstraint>) -> anyhow::Result<Dom> {
        build(
            |fut| {
                let task = self.spawn(fut);
                task.detach();
            },
            builder,
        )
    }
}

// Helper function for defining `ListPatchApply for Dom`.
fn list_patch_apply_node(
    self_node: &mut web_sys::Node,
    patch: ListPatch<web_sys::Node>,
) -> Vec<web_sys::Node> {
    let mut removed = vec![];
    match patch {
        ListPatch::Splice {
            range,
            replace_with,
        } => {
            let mut replace_with = replace_with.into_iter();
            let list: web_sys::NodeList = self_node.child_nodes();
            let children: Vec<web_sys::Node> =
                (0..list.length()).filter_map(|i| list.get(i)).collect();

            let start_index = match range.0 {
                Bound::Included(i) => i,
                Bound::Excluded(i) => i,
                Bound::Unbounded => 0,
            };
            let end_index = match range.1 {
                Bound::Included(i) => i,
                Bound::Excluded(i) => i,
                Bound::Unbounded => (list.length() as usize).max(1) - 1,
            };

            let mut child_after = None;
            for i in start_index..=end_index {
                if let Some(old_child) = children.get(i) {
                    if range.contains(&i) {
                        if let Some(new_child) = replace_with.next() {
                            self_node.replace_child(&new_child, &old_child).unwrap();
                        } else {
                            self_node.remove_child(&old_child).unwrap();
                        }
                        removed.push(old_child.clone());
                    } else {
                        child_after = Some(old_child);
                    }
                }
            }

            for child in replace_with {
                self_node.insert_before(&child, child_after).unwrap();
            }
        }
        ListPatch::Push(new_node) => {
            let _ = self_node.append_child(&new_node).unwrap();
        }
        ListPatch::Pop => {
            if let Some(child) = self_node.last_child() {
                let _ = self_node.remove_child(&child).unwrap();
                removed.push(child);
            }
        }
    }
    removed
}

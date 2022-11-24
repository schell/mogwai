//! DOM building.
use std::{marker::PhantomData, pin::Pin};

use async_executor::{Executor, LocalExecutor};
use futures::Future;
pub use mogwai::{builder::*, traits::{ConstraintType, ConstrainedStream, ConstrainedFuture}};
use mogwai::{
    futures::StreamExt,
    patch::{HashPatch, ListPatch},
};

use crate::view::Dom;

/// Set all the initial values of a Dom node.
pub fn set_initial_values<C: ConstraintType>(
    dom: &Dom,
    texts: impl Iterator<Item = String>,
    attribs: impl Iterator<Item = HashPatch<String, String>>,
    bool_attribs: impl Iterator<Item = HashPatch<String, bool>>,
    styles: impl Iterator<Item = HashPatch<String, String>>,
    children: impl Iterator<Item = ListPatch<ViewBuilder<Dom, C>>>,
) -> Result<(), anyhow::Error> {
    for text in texts {
        dom.set_text(&text).map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    for patch in attribs {
        dom.patch_attribs(patch)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    for patch in bool_attribs {
        dom.patch_bool_attribs(patch)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    for patch in styles {
        dom.patch_styles(patch)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    for patch in children {
        dom.build_and_patch_children(patch)?;
    }

    Ok(())
}

/// Set all the streaming values of a Dom node, spawning async logic loops that update
/// values as items come down the streams.
pub fn set_streaming_values<'a, C: ConstraintType>(
    spawn: impl Fn(ConstrainedFuture<(), C>),
    node: &Dom,
    mut text_stream: ConstrainedStream<String, C>,
    mut attrib_stream: ConstrainedStream<HashPatch<String, String>, C>,
    mut bool_attrib_stream: ConstrainedStream<HashPatch<String, bool>, C>,
    mut style_stream: ConstrainedStream<HashPatch<String, String>, C>,
    mut child_stream: ConstrainedStream<ListPatch<ViewBuilder<Dom, C>>, C>,
) -> Result<(), String> {
    let text_node = node.clone();
    spawn(Box::pin(async move {
        while let Some(msg) = text_stream.next().await {
            text_node.set_text(&msg).unwrap();
        }
    }));

    let attrib_node = node.clone();
    spawn(Box::pin(async move {
        while let Some(patch) = attrib_stream.next().await {
            attrib_node.patch_attribs(patch).unwrap();
        }
    }));

    let bool_attrib_node = node.clone();
    spawn(Box::pin(async move {
        while let Some(patch) = bool_attrib_stream.next().await {
            bool_attrib_node.patch_bool_attribs(patch).unwrap();
        }
    }));

    let style_node = node.clone();
    spawn(Box::pin(async move {
        while let Some(patch) = style_stream.next().await {
            style_node.patch_styles(patch).unwrap();
        }
    }));

    let parent_node = node.clone();
    spawn(Box::pin(async move {
        while let Some(patch) = child_stream.next().await {
            let _ = parent_node.build_and_patch_children(patch);
        }
    }));

    Ok(())
}

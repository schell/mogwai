//! DOM building.


use mogwai_core::{builder::{
        AttribStream, BooleanAttribStream, ChildStream, StyleStream, TextStream, ViewBuilder,
    }, futures::StreamExt, patch::{HashPatch, ListPatch}, target::spawn};

use crate::view::Dom;

/// Set all the initial values of a Dom node.
pub async fn set_initial_values(
    dom: &Dom,
    texts: impl Iterator<Item = String>,
    attribs: impl Iterator<Item = HashPatch<String, String>>,
    bool_attribs: impl Iterator<Item = HashPatch<String, bool>>,
    styles: impl Iterator<Item = HashPatch<String, String>>,
    children: impl Iterator<Item = ListPatch<ViewBuilder<Dom>>>,
) -> Result<(), anyhow::Error> {
    for text in texts {
        dom.set_text(&text).map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    for patch in attribs {
        dom.patch_attribs(patch).map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    for patch in bool_attribs {
        dom.patch_bool_attribs(patch).map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    for patch in styles {
        dom.patch_styles(patch).map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    for patch in children {
        dom.build_and_patch_children(patch).await?;
    }

    Ok(())
}

/// Set all the streaming values of a Dom node.
pub fn set_streaming_values(
    node: &Dom,
    mut text_stream: TextStream,
    mut attrib_stream: AttribStream,
    mut bool_attrib_stream: BooleanAttribStream,
    mut style_stream: StyleStream,
    mut child_stream: ChildStream<Dom>,
) -> Result<(), String> {
    let text_node = node.clone();
    spawn(async move {
        while let Some(msg) = text_stream.next().await {
            text_node.set_text(&msg).unwrap();
        }
    });

    let attrib_node = node.clone();
    spawn(async move {
        while let Some(patch) = attrib_stream.next().await {
            attrib_node.patch_attribs(patch).unwrap();
        }
    });

    let bool_attrib_node = node.clone();
    spawn(async move {
        while let Some(patch) = bool_attrib_stream.next().await {
            bool_attrib_node.patch_bool_attribs(patch).unwrap();
        }
    });

    let style_node = node.clone();
    spawn(async move {
        while let Some(patch) = style_stream.next().await {
            style_node.patch_styles(patch).unwrap();
        }
    });

    let parent_node = node.clone();
    spawn(async move {
        while let Some(patch) = child_stream.next().await {
            let _ = parent_node.build_and_patch_children(patch).await;
        }
    });

    Ok(())
}

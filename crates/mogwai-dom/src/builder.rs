//! DOM building.

use mogwai_core::{
    builder::{
        AttribStream, BooleanAttribStream, ChildStream, StyleStream, TextStream, ViewBuilder,
    },
    patch::{HashPatch, ListPatch},
    target::spawn,
    view::View,
    futures::StreamExt
};

use crate::view::Dom;

/// Set all the initial values of a Dom node.
pub fn set_initial_values(
    node: &Dom,
    texts: impl Iterator<Item = String>,
    attribs: impl Iterator<Item = HashPatch<String, String>>,
    bool_attribs: impl Iterator<Item = HashPatch<String, bool>>,
    styles: impl Iterator<Item = HashPatch<String, String>>,
    children: impl Iterator<Item = ListPatch<ViewBuilder<Dom>>>,
) -> Result<(), String> {
    for text in texts {
        node.set_text(&text)?;
    }

    for patch in attribs {
        node.patch_attribs(patch)?;
    }

    for patch in bool_attribs {
        node.patch_bool_attribs(patch)?;
    }

    for patch in styles {
        node.patch_styles(patch)?;
    }

    for patch in children {
        let patch = patch.map(|vb| View::try_from(vb).unwrap().into_inner());
        node.patch_children(patch)?;
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
            let patch = patch.map(|vb| View::try_from(vb).unwrap().into_inner());
            parent_node.patch_children(patch).unwrap();
        }
    });

    Ok(())
}

//! Contains parsing an RSX node into various data types.
use std::convert::TryFrom;

use proc_macro2::Span;
use quote::quote;
use syn::Error;
use syn_rsx::{Node, NodeType};

fn under_to_dash(s: &str) -> String {
    s.replace("_", "-")
}

#[derive(Clone, Debug)]
/// An enumeration of all supported attribute types.
pub enum AttributeToken {
    CastType(syn::Expr),
    PostBuild(syn::Expr),
    CaptureView(syn::Expr),
    Xmlns(syn::Expr),
    Style(syn::Expr),
    StyleSingle(String, syn::Expr),
    On(String, syn::Expr),
    Window(String, syn::Expr),
    Document(String, syn::Expr),
    BooleanSingle(String, syn::Expr),
    BooleanTrue(String),
    PatchChildren(syn::Expr),
    Attrib(String, syn::Expr),
}

impl TryFrom<syn_rsx::Node> for AttributeToken {
    type Error = syn::Error;

    fn try_from(node: syn_rsx::Node) -> Result<Self, Self::Error> {
        let span = node.name_span().unwrap_or(Span::call_site());
        if let Some(key) = node.name_as_string() {
            if let Some(expr) = node.value {
                match key.split(':').collect::<Vec<_>>().as_slice() {
                    ["cast", "type"] => Ok(AttributeToken::CastType(expr)),
                    ["post", "build"] => Ok(AttributeToken::PostBuild(expr)),
                    ["capture", "view"] => Ok(AttributeToken::CaptureView(expr)),
                    ["xmlns"] => Ok(AttributeToken::Xmlns(expr)),
                    ["style"] => Ok(AttributeToken::Style(expr)),
                    ["style", name] => {
                        let name = under_to_dash(name);
                        Ok(AttributeToken::StyleSingle(name, expr))
                    }
                    ["on", event] => Ok(AttributeToken::On(event.to_string(), expr)),
                    ["window", event] => Ok(AttributeToken::Window(event.to_string(), expr)),
                    ["document", event] => Ok(AttributeToken::Document(event.to_string(), expr)),
                    ["boolean", name] => {
                        let name = under_to_dash(name);
                        Ok(AttributeToken::BooleanSingle(name, expr))
                    }
                    ["patch", "children"] => Ok(AttributeToken::PatchChildren(expr)),
                    [attribute_name] => {
                        let name = under_to_dash(attribute_name);
                        Ok(AttributeToken::Attrib(name, expr))
                    }
                    keys => {
                        let name = under_to_dash(&keys.join(":"));
                        Ok(AttributeToken::Attrib(name, expr))
                    }
                }
            } else {
                let name = under_to_dash(&key);
                Ok(AttributeToken::BooleanTrue(name))
            }
        } else {
            Err(Error::new(span, "dom attribute is missing a name"))
        }
    }
}

impl AttributeToken {
    /// Attempt to create a token stream representing one link in a `ViewBuilder` chain.
    pub fn try_builder_token_stream(
        self: &AttributeToken,
    ) -> Result<proc_macro2::TokenStream, Error> {
        use AttributeToken::*;
        match self {
            CastType(_) => Ok(quote! {}), // handled by a preprocessor
            PostBuild(expr) => Ok(quote! {
                .with_post_build(#expr)
            }),
            CaptureView(expr) => Ok(quote! {
                .with_capture_view(#expr)
            }),
            Xmlns(expr) => Ok(quote! {
                .with_namespace(#expr)
            }),
            Style(expr) => Ok(quote! {
                .with_style_stream(#expr)
            }),
            StyleSingle(name, expr) => Ok(quote! {
                .with_single_style_stream(#name, #expr)
            }),
            On(name, expr) => Ok(quote! {
                .with_event(#name, mogwai::core::event::EventTargetType::Myself, #expr)
            }),
            Window(name, expr) => Ok(quote! {
                .with_event(#name, mogwai::core::event::EventTargetType::Window, #expr)
            }),
            Document(name, expr) => Ok(quote! {
                .with_event(#name, mogwai::core::event::EventTargetType::Document, #expr)
            }),
            BooleanSingle(name, expr) => Ok(quote! {
                .with_single_bool_attrib_stream(#name, #expr)
            }),
            PatchChildren(expr) => Ok(quote! {
                .with_child_stream(#expr)
            }),
            Attrib(name, expr) => Ok(quote! {
                .with_single_attrib_stream(#name, #expr)
            }),
            BooleanTrue(expr) => Ok(quote! {
                .with_single_bool_attrib_stream(#expr, true)
            }),
        }
    }
}

#[derive(Clone, Debug)]
/// An enumeration of all supported nodes types.
pub enum ViewToken {
    Element {
        name: String,
        name_span: proc_macro2::Span,
        attributes: Vec<AttributeToken>,
        children: Vec<ViewToken>,
    },
    Text(syn::Expr),
    Block(syn::Expr),
}

impl TryFrom<Node> for ViewToken {
    type Error = Error;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        let name_span = node.name_span().unwrap_or(Span::call_site());
        match &node.node_type {
            NodeType::Element => match node.name_as_string() {
                Some(tag) => {
                    let name = tag;

                    let mut attributes = vec![];
                    for attribute in node.attributes.into_iter() {
                        let token = AttributeToken::try_from(attribute)?;
                        attributes.push(token);
                    }

                    let mut children = vec![];
                    for child in node.children.into_iter() {
                        let token = ViewToken::try_from(child)?;
                        children.push(token);
                    }

                    Ok(ViewToken::Element {
                        name,
                        name_span,
                        attributes,
                        children,
                    })
                }
                None => Err(Error::new(
                    node.name_span().unwrap_or_else(|| Span::call_site()),
                    "View node is missing a name.",
                )),
            },
            NodeType::Text => {
                if let Some(val) = node.value {
                    Ok(ViewToken::Text(val))
                } else {
                    Err(Error::new(
                        node.name_span().unwrap_or(Span::call_site()),
                        "Text node is missing a value.",
                    ))
                }
            }
            NodeType::Block => {
                if let Some(val) = node.value {
                    Ok(ViewToken::Block(val))
                } else {
                    Err(Error::new(
                        node.name_span().unwrap_or(Span::call_site()),
                        "Block node is missing a value.",
                    ))
                }
            }
            _ => Err(Error::new(
                node.name_span().unwrap_or_else(|| Span::call_site()),
                "View node is missing a name.",
            )),
        }
    }
}

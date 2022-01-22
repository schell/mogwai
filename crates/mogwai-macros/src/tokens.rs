//! Contains parsing an RSX node into various data types.
use std::convert::TryFrom;

use proc_macro2::Span;
use quote::quote;
use syn::{
    parse::Parse, punctuated::Punctuated, token, Error, Expr, Ident,
    LitStr, Token,
};
use syn_rsx::{Node, NodeType};

fn under_to_dash(s: impl AsRef<str>) -> String {
    s.as_ref().replace("_", "-")
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
                let keys = key.split(':').collect::<Vec<_>>();
                Ok(AttributeToken::from_keys_expr_pair(&keys, expr))
            } else {
                let name = under_to_dash(&key);
                Ok(AttributeToken::BooleanTrue(name))
            }
        } else {
            Err(Error::new(span, "dom attribute is missing a name"))
        }
    }
}

impl Parse for AttributeToken {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut keys: Vec<String> = vec![];
        while !input.lookahead1().peek(Token![=]) && !input.is_empty() {
            let key_segment = match input.parse::<Ident>() {
                Ok(ident) => Ok(format!("{}", ident)),
                Err(e1) => {
                    if input.parse::<Token![type]>().is_ok() {
                        Ok("type".to_string())
                    } else {
                        Err(e1)
                    }
                }
            }?;
            let _ = input.parse::<Option<Token![:]>>()?;
            keys.push(key_segment);
        }
        if input.parse::<Token![=]>().is_ok() {
            let expr = input.parse::<Expr>()?;
            Ok(AttributeToken::from_keys_expr_pair(&keys, expr))
        } else {
            let key = under_to_dash(keys.join(":"));
            Ok(AttributeToken::BooleanTrue(key))
        }
    }
}

impl AttributeToken {
    pub fn from_keys_expr_pair(keys: &[impl AsRef<str>], expr: Expr) -> Self {
        let ks = keys.iter().map(|s| s.as_ref()).collect::<Vec<_>>();
        match ks.as_slice() {
            ["cast", "type"] => AttributeToken::CastType(expr),
            ["post", "build"] => AttributeToken::PostBuild(expr),
            ["capture", "view"] => AttributeToken::CaptureView(expr),
            ["xmlns"] => AttributeToken::Xmlns(expr),
            ["style"] => AttributeToken::Style(expr),
            ["style", name] => {
                let name = under_to_dash(name);
                AttributeToken::StyleSingle(name, expr)
            }
            ["on", event] => AttributeToken::On(event.to_string(), expr),
            ["window", event] => AttributeToken::Window(event.to_string(), expr),
            ["document", event] => AttributeToken::Document(event.to_string(), expr),
            ["boolean", name] => {
                let name = under_to_dash(name);
                AttributeToken::BooleanSingle(name, expr)
            }
            ["patch", "children"] => AttributeToken::PatchChildren(expr),
            [attribute_name] => {
                let name = under_to_dash(attribute_name);
                AttributeToken::Attrib(name, expr)
            }
            keys => {
                let name = under_to_dash(&keys.join(":"));
                AttributeToken::Attrib(name, expr)
            }
        }
    }
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

impl Parse for ViewToken {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(token::Brace) {
            Ok(ViewToken::Block(input.parse::<syn::Expr>()?))
        } else if lookahead.peek(LitStr) {
            Ok(ViewToken::Text(input.parse::<syn::Expr>()?))
        } else {
            let tag: Ident = input.parse()?;
            let attributes = if input.lookahead1().peek(token::Paren) {
                let paren_content;
                let _paren_token: token::Paren = syn::parenthesized!(paren_content in input);
                let attrs: Punctuated<AttributeToken, Token![,]> =
                    paren_content.parse_terminated(AttributeToken::parse)?;
                attrs.into_iter().collect::<Vec<_>>()
            } else {
                vec![]
            };

            let brace_content;
            let _brace: token::Brace = syn::braced!(brace_content in input);
            let children: ViewTokens = brace_content.parse()?;

            Ok(ViewToken::Element {
                name: format!("{}", tag),
                name_span: tag.span(),
                attributes,
                children: children.views,
            })
        }
    }
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

/// A list of view tokens
#[derive(Default)]
pub struct ViewTokens {
    pub views: Vec<ViewToken>,
}

impl Parse for ViewTokens {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut tokens: ViewTokens = ViewTokens::default();
        while !input.is_empty() {
            tokens.views.push(input.parse::<ViewToken>()?);
        }
        Ok(tokens)
    }
}

//! Contains parsing an RSX node into various data types.
use std::{collections::HashMap, marker::PhantomData, str::FromStr};

use quote::{ToTokens, quote};
use syn::{Expr, Ident, Token, parse::Parse};

fn under_to_dash(s: impl AsRef<str>) -> String {
    s.as_ref().trim_matches('_').replace('_', "-")
}

/// Parses `let my_ident: MyType =`
#[derive(Debug, Clone)]
pub struct LetIdent {
    ident: Ident,
    cast: Option<syn::Type>,
}

impl Parse for LetIdent {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<syn::token::Let>()?;
        let ident = input.parse::<Ident>()?;
        let cast = {
            let lookahead = input.lookahead1();
            if lookahead.peek(syn::Token![:]) {
                input.parse::<syn::Token![:]>()?;
                let ty = input.parse::<syn::Type>()?;
                Some(ty)
            } else {
                None
            }
        };
        let _ = input.parse::<Token![=]>()?;

        Ok(Self { ident, cast })
    }
}

#[derive(Debug)]
/// An enumeration of all supported nodes types.
pub enum ViewToken {
    Element {
        name: String,
        ident: Option<LetIdent>,
        attributes: Vec<AttributeToken>,
        children: Vec<ViewToken>,
    },
    Text {
        ident: Option<LetIdent>,
        expr: syn::Expr,
    },
    Block {
        ident: Option<LetIdent>,
        expr: syn::Expr,
    },
}

impl Parse for ViewToken {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident = if input.lookahead1().peek(syn::token::Let) {
            Some(input.parse::<LetIdent>()?)
        } else {
            None
        };

        let lookahead = input.lookahead1();
        if lookahead.peek(syn::token::Brace) {
            let braced_content;
            let _ = syn::braced!(braced_content in input);
            let expr: syn::Expr = braced_content.parse()?;
            Ok(ViewToken::Block { ident, expr })
        } else if lookahead.peek(syn::LitStr) {
            Ok(ViewToken::Text {
                ident,
                expr: input.parse::<syn::Expr>()?,
            })
        } else {
            let tag: Ident = input.parse()?;
            let attributes = if input.lookahead1().peek(syn::token::Paren) {
                let paren_content;
                let _paren_token: syn::token::Paren = syn::parenthesized!(paren_content in input);
                let attrs: syn::punctuated::Punctuated<AttributeToken, Token![,]> =
                    paren_content.parse_terminated(AttributeToken::parse, syn::Token![,])?;
                attrs.into_iter().collect::<Vec<_>>()
            } else {
                vec![]
            };

            let brace_content;
            let _brace: syn::token::Brace = syn::braced!(brace_content in input);
            let mut children: Vec<ViewToken> = vec![];
            while !brace_content.is_empty() {
                children.push(brace_content.parse::<ViewToken>()?);
            }

            Ok(ViewToken::Element {
                name: format!("{}", tag),
                ident,
                attributes,
                children,
            })
        }
    }
}

/// A trait for specifying the output of ViewToken.
///
/// This allows separating the output of an rsx! style macro into N separate
/// paths, all with the same structure.
trait Flavor {
    fn create_text(ident: &syn::Ident, expr: &syn::Expr) -> proc_macro2::TokenStream;
    fn create_element(el: &String) -> proc_macro2::TokenStream;
    fn create_element_ns(el: &String, ns: &syn::Expr) -> proc_macro2::TokenStream;
    fn cast_creation(
        ident: &syn::Ident,
        expr: &syn::Type,
        creation: proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream;
    fn append_child(ident: &syn::Ident, child_id: &syn::Ident) -> proc_macro2::TokenStream;
    fn set_style_property(
        ident: &syn::Ident,
        key: &String,
        expr: &syn::Expr,
    ) -> proc_macro2::TokenStream;
    fn set_attribute(
        ident: &syn::Ident,
        key: &String,
        expr: &syn::Expr,
    ) -> proc_macro2::TokenStream;
    fn create_listener(
        ident: &syn::Ident,
        listener: &syn::Expr,
        event: &String,
    ) -> proc_macro2::TokenStream;
    fn create_window_listener(listener: &syn::Expr, event: &String) -> proc_macro2::TokenStream;
    fn create_document_listener(listener: &syn::Expr, event: &String) -> proc_macro2::TokenStream;
}

pub struct WebSysFlavor;

impl Flavor for WebSysFlavor {
    fn create_text(ident: &syn::Ident, expr: &syn::Expr) -> proc_macro2::TokenStream {
        quote! { let #ident = web_sys::Text::new_with_data(#expr).unwrap(); }
    }
    fn create_element_ns(el: &String, ns: &syn::Expr) -> proc_macro2::TokenStream {
        quote! {
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .create_element_ns(#el, #ns)
                .unwrap()
        }
    }

    fn create_element(el: &String) -> proc_macro2::TokenStream {
        quote! {
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .create_element(#el)
                .unwrap()
        }
    }

    fn cast_creation(
        ident: &syn::Ident,
        cast_expr: &syn::Type,
        creation: proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        quote! {
            let #ident: #cast_expr = #creation
                .dyn_into::<#cast_expr>()
                .unwrap();
        }
    }

    fn append_child(ident: &syn::Ident, child_id: &syn::Ident) -> proc_macro2::TokenStream {
        quote! { let _ = #ident.append_child(#child_id.as_ref()).unwrap(); }
    }

    fn set_style_property(
        ident: &syn::Ident,
        key: &String,
        expr: &syn::Expr,
    ) -> proc_macro2::TokenStream {
        quote! { #ident.dyn_ref::<web_sys::HtmlElement>().unwrap().style().set_property(#key, #expr).unwrap(); }
    }

    fn set_attribute(
        ident: &syn::Ident,
        key: &String,
        expr: &syn::Expr,
    ) -> proc_macro2::TokenStream {
        quote! { #ident.set_attribute(#key, #expr).unwrap(); }
    }

    fn create_listener(
        ident: &syn::Ident,
        listener: &syn::Expr,
        event: &String,
    ) -> proc_macro2::TokenStream {
        quote! {
            let #listener = mogwai_futura::web::event::EventListener::new(
                &#ident,
                #event,
            );
        }
    }

    fn create_window_listener(listener: &syn::Expr, event: &String) -> proc_macro2::TokenStream {
        quote! {
            let #listener = mogwai_futura::web::event::EventListener::new(
                web_sys::window().unwrap(),
                #event
            );
        }
    }

    fn create_document_listener(listener: &syn::Expr, event: &String) -> proc_macro2::TokenStream {
        quote! {
            let #listener = mogwai_futurea::web::event::EventListener::new(
                web_sys::window().unwrap().document().unwrap(),
                #event
            );
        }
    }
}

pub struct SsrFlavor;

impl Flavor for SsrFlavor {
    fn create_text(ident: &syn::Ident, expr: &syn::Expr) -> proc_macro2::TokenStream {
        quote! { let #ident = mogwai_futura::ssr::Text::new(#expr); }
    }

    fn create_element(el: &String) -> proc_macro2::TokenStream {
        quote! { mogwai_futura::ssr::Container::new(#el) }
    }

    fn create_element_ns(el: &String, ns: &syn::Expr) -> proc_macro2::TokenStream {
        quote! {{
            let __container = mogwai_futura::ssr::Container::new(#el);
            __container.set_property("xmlns", #ns);
            __container
        }}
    }

    fn cast_creation(
        ident: &syn::Ident,
        _expr: &syn::Type,
        creation: proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        quote! { let #ident = #creation; }
    }

    fn append_child(ident: &syn::Ident, child_id: &syn::Ident) -> proc_macro2::TokenStream {
        quote! {
            #ident.append_child(#child_id.clone().into());
        }
    }

    fn set_style_property(
        ident: &syn::Ident,
        key: &String,
        expr: &syn::Expr,
    ) -> proc_macro2::TokenStream {
        quote! { #ident.set_style(#key, #expr); }
    }

    fn set_attribute(
        ident: &syn::Ident,
        key: &String,
        expr: &syn::Expr,
    ) -> proc_macro2::TokenStream {
        quote! { #ident.set_property(#key, #expr); }
    }

    fn create_listener(
        _ident: &syn::Ident,
        listener: &syn::Expr,
        event: &String,
    ) -> proc_macro2::TokenStream {
        SsrFlavor::create_window_listener(listener, event)
    }

    fn create_window_listener(listener: &syn::Expr, event: &String) -> proc_macro2::TokenStream {
        quote! {
            let _event = #event;
            let (tx, rx) = async_channel::bounded(1);
            let #listener = mogwai_futura::ssr::EventListener { tx, rx };
        }
    }

    fn create_document_listener(listener: &syn::Expr, event: &String) -> proc_macro2::TokenStream {
        SsrFlavor::create_window_listener(listener, event)
    }
}

pub struct BuilderFlavor;

pub struct ViewTokenOutput<'a, T> {
    view: &'a ViewToken,
    _phantom: PhantomData<T>,
}

impl<'a, T> ViewTokenOutput<'a, T> {
    pub fn new(view: &'a ViewToken) -> Self {
        Self {
            view,
            _phantom: PhantomData,
        }
    }
}

impl<T: Flavor> quote::ToTokens for ViewTokenOutput<'_, T> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.view.to_named_tokens::<T>(None::<String>, 0, tokens);
    }
}

impl ViewToken {
    fn leaf_name(&self) -> &str {
        match self {
            ViewToken::Element { name, .. } => name,
            ViewToken::Text { .. } => "text",
            ViewToken::Block { .. } => "block",
        }
    }

    fn to_named_tokens<T: Flavor>(
        &self,
        parent_name: Option<impl AsRef<str>>,
        index: usize,
        tokens: &mut proc_macro2::TokenStream,
    ) -> LetIdent {
        let n = if index == 0 {
            String::new()
        } else {
            format!("{index}")
        };

        let parent_name = parent_name
            .map(|name| format!("{}_", name.as_ref()))
            .unwrap_or_default();
        let name = format!("{parent_name}{}{n}", self.leaf_name());
        let generic_id = LetIdent {
            ident: quote::format_ident!("_{name}"),
            cast: None,
        };

        match self {
            ViewToken::Element {
                name: el,
                ident,
                attributes,
                children,
            } => {
                let let_ident = ident.clone().unwrap_or(generic_id);
                let LetIdent { ident, cast } = let_ident.clone();
                let creation = attributes
                    .iter()
                    .find_map(|att| {
                        if let AttributeToken::Xmlns(ns) = att {
                            Some(T::create_element_ns(&el, &ns))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| T::create_element(el));
                if let Some(cast_expr) = cast {
                    T::cast_creation(&ident, &cast_expr, creation).to_tokens(tokens);
                } else {
                    quote! {
                        let #ident = #creation;
                    }
                    .to_tokens(tokens);
                }

                let mut indices = HashMap::<&str, usize>::new();
                for child in children.iter() {
                    let index = indices
                        .entry(child.leaf_name())
                        .and_modify(|i| {
                            *i += 1;
                        })
                        .or_insert(0);
                    let child_id = child
                        .to_named_tokens::<T>(Some(name.as_str()), *index, tokens)
                        .ident;
                    T::append_child(&ident, &child_id).to_tokens(tokens);
                }
                for att in attributes.iter() {
                    match att {
                        AttributeToken::Let(outside_id) => {
                            quote! { #outside_id = #ident; }.to_tokens(tokens);
                        }
                        AttributeToken::StyleSingle(key, expr) => {
                            T::set_style_property(&ident, key, expr).to_tokens(tokens);
                        }
                        AttributeToken::Attrib(key, expr) => {
                            T::set_attribute(&ident, key, expr).to_tokens(tokens);
                        }
                        AttributeToken::On(event, listener) => {
                            T::create_listener(&ident, listener, event).to_tokens(tokens);
                        }
                        AttributeToken::Xmlns(_) => {
                            // handled elsewhere
                        }
                        AttributeToken::Window(event, listener) => {
                            T::create_window_listener(listener, event).to_tokens(tokens);
                        }
                        AttributeToken::Document(event, listener) => {
                            T::create_document_listener(listener, event).to_tokens(tokens);
                        }
                    }
                }
                let_ident
            }
            ViewToken::Text { ident, expr } => {
                let let_ident = ident.clone().unwrap_or(generic_id);
                let id = let_ident.ident.clone();
                T::create_text(&id, expr).to_tokens(tokens);
                let_ident
            }
            ViewToken::Block { ident, expr } => {
                let let_ident = ident.clone().unwrap_or(generic_id);
                let id = let_ident.ident.clone();
                quote! { let #id = #expr; }.to_tokens(tokens);
                let_ident
            }
        }
    }
}

#[derive(Clone, Debug)]
/// An enumeration of all supported attribute types.
pub enum AttributeToken {
    Let(Ident),
    Xmlns(syn::Expr),
    // TODO: allow the name to be syn::Expr
    StyleSingle(String, syn::Expr),
    On(String, syn::Expr),
    Window(String, syn::Expr),
    Document(String, syn::Expr),
    Attrib(String, syn::Expr),
}

impl Parse for AttributeToken {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut keys: Vec<String> = vec![];
        while !input.lookahead1().peek(Token![=])
            && !input.lookahead1().peek(Token![,])
            && !input.is_empty()
        {
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
        } else if keys.len() == 1 {
            let ident = quote::format_ident!("{}", keys[0]);
            Ok(AttributeToken::Let(ident))
        } else {
            let key = under_to_dash(keys.join(":"));
            let none: syn::Expr =
                syn::parse2(proc_macro2::TokenStream::from_str("None").unwrap()).unwrap();
            Ok(AttributeToken::Attrib(key, none))
        }
    }
}

impl AttributeToken {
    pub fn from_keys_expr_pair(keys: &[impl AsRef<str>], expr: Expr) -> Self {
        let ks = keys.iter().map(|s| s.as_ref()).collect::<Vec<_>>();
        match ks.as_slice() {
            ["xmlns"] => AttributeToken::Xmlns(expr),
            ["style", name] => {
                let name = under_to_dash(name);
                AttributeToken::StyleSingle(name, expr)
            }
            ["on", event] => AttributeToken::On(event.to_string(), expr),
            ["window", event] => AttributeToken::Window(event.to_string(), expr),
            ["document", event] => AttributeToken::Document(event.to_string(), expr),
            [attribute_name] => {
                let name = under_to_dash(attribute_name);
                AttributeToken::Attrib(name, expr)
            }
            keys => {
                let name = under_to_dash(keys.join(":"));
                AttributeToken::Attrib(name, expr)
            }
        }
    }
}

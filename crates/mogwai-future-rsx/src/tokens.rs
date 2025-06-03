//! Contains parsing an RSX node into various data types.
use std::{collections::HashMap, str::FromStr};

use quote::{ToTokens, quote};
use syn::{Expr, Ident, Token, parse::Parse, spanned::Spanned};

fn under_to_dash(s: impl AsRef<str>) -> String {
    s.as_ref().trim_matches('_').replace('_', "-")
}

/// Matches `proxy (model) => model.id.to_string()` where
/// * `proxy_ident`: `proxy`
/// * `pattern`: `model`
/// * `expr`: `model.id.to_string()`
#[derive(Clone, Debug)]
pub struct ProxyUpdate {
    proxy_ident: syn::Ident,
    update_ident: Option<syn::Ident>,
    pattern: syn::Pat,
    expr: syn::Expr,
}

impl Parse for ProxyUpdate {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let proxy_ident: syn::Ident = input.parse()?;
        let content;
        let _ = syn::parenthesized!(content in input);
        let pattern = syn::Pat::parse_single(&content)?;
        let _ = content.parse::<Token![=]>()?;
        let _ = content.parse::<Token![>]>()?;
        let expr: syn::Expr = content.parse()?;
        Ok(ProxyUpdate {
            proxy_ident,
            update_ident: None,
            pattern,
            expr,
        })
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ProxyUpdateKey {
    Attrib(String),
    Block {
        parent: syn::Ident,
        block: syn::Ident,
    },
}

/// Used to create a proxy "on_update" call at the end of an rsx macro.
pub struct ProxyOnUpdate {
    updated_idents: Vec<syn::Ident>,
    updates: HashMap<ProxyUpdateKey, ProxyUpdate>,
}

impl ToTokens for ProxyOnUpdate {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let clones = self
            .updated_idents
            .iter()
            .map(|ident| {
                quote! {
                    let #ident = #ident.clone();
                }
            })
            .collect::<Vec<_>>();
        let updates = self
            .updates
            .iter()
            .map(|(key, update)| match key {
                ProxyUpdateKey::Attrib(name) => {
                    let ident = &update.update_ident;
                    let pat = &update.pattern;
                    let expr = &update.expr;
                    quote! {{
                            let #pat = model;
                            #ident.set_property(#name, #expr);
                    }}
                }
                ProxyUpdateKey::Block { parent, block } => {
                    let pat = &update.pattern;
                    let expr = &update.expr;
                    quote! {{
                            let #pat = model;
                            #block.replace(#parent, #expr);
                    }}
                }
            })
            .collect::<Vec<_>>();
        quote! {{
            #(#clones)*
            move |model| {
                #(#updates)*
            }
        }}
        .to_tokens(tokens);
    }
}

fn insert_proxy(
    proxies: &mut HashMap<syn::Ident, ProxyOnUpdate>,
    ident: &syn::Ident,
    key: ProxyUpdateKey,
    proxy_update: &ProxyUpdate,
) {
    let mut proxy_update = proxy_update.clone();
    proxy_update.update_ident = Some(ident.clone());
    let proxy_on_update = proxies
        .entry(proxy_update.proxy_ident.clone())
        .or_insert_with(|| ProxyOnUpdate {
            updated_idents: vec![],
            updates: HashMap::default(),
        });
    proxy_on_update.updated_idents.push(ident.clone());
    proxy_on_update.updates.insert(key, proxy_update);
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
    BlockExpr {
        ident: Option<LetIdent>,
        expr: syn::Expr,
    },
    BlockProxy {
        ident: Option<LetIdent>,
        proxy: ProxyUpdate,
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

            if braced_content.fork().parse::<ProxyUpdate>().is_ok() {
                let mut proxy = braced_content.parse::<ProxyUpdate>()?;
                proxy.update_ident = ident.as_ref().map(|lid| lid.ident.clone());
                Ok(ViewToken::BlockProxy { ident, proxy })
            } else {
                let expr: syn::Expr = braced_content.parse()?;
                Ok(ViewToken::BlockExpr { ident, expr })
            }
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

pub struct WebFlavor;

impl WebFlavor {
    fn create_text(ident: &syn::Ident, expr: &syn::Expr) -> proc_macro2::TokenStream {
        quote! { let #ident = V::Text::new(#expr); }
    }
    fn create_element_ns(el: &str, ns: &syn::Expr) -> proc_macro2::TokenStream {
        quote! { V::Element::new_namespace(#el, #ns) }
    }

    fn create_element(el: &str) -> proc_macro2::TokenStream {
        quote! { V::Element::new(#el) }
    }

    fn append_child(ident: &syn::Ident, child_id: &syn::Ident) -> proc_macro2::TokenStream {
        quote! { #ident.append_child(&#child_id); }
    }

    fn set_style_property(
        ident: &syn::Ident,
        key: &str,
        expr: &syn::Expr,
    ) -> proc_macro2::TokenStream {
        quote! { #ident.set_style(#key, #expr); }
    }

    fn set_attribute(ident: &syn::Ident, key: &str, expr: &syn::Expr) -> proc_macro2::TokenStream {
        quote! { #ident.set_property(#key, #expr); }
    }

    fn set_attribute_proxy(
        ident: &syn::Ident,
        key: &str,
        proxy: &ProxyUpdate,
    ) -> proc_macro2::TokenStream {
        let proxy_ident = &proxy.proxy_ident;
        let pattern = &proxy.pattern;
        let expr = &proxy.expr;
        quote! { #ident.set_property(#key, {
            let #pattern = #proxy_ident;
            #expr
        })}
    }

    fn create_listener(
        ident: &syn::Ident,
        listener: &syn::Expr,
        event: &str,
    ) -> proc_macro2::TokenStream {
        quote! { let #listener = #ident.listen(#event); }
    }

    fn create_window_listener(listener: &syn::Expr, event: &str) -> proc_macro2::TokenStream {
        quote! { let #listener = V::EventListener::on_window( #event ); }
    }

    fn create_document_listener(listener: &syn::Expr, event: &str) -> proc_macro2::TokenStream {
        quote! { let #listener = V::EventListener::on_document( #event ); }
    }

    fn proxy_child(
        parent: &syn::Ident,
        ident: &syn::Ident,
        proxy: &ProxyUpdate,
    ) -> proc_macro2::TokenStream {
        let proxy_ident = &proxy.proxy_ident;
        let pattern = &proxy.pattern;
        let expr = &proxy.expr;
        quote! { let #ident = {
            let #pattern = &#proxy_ident;
            mogwai_futura::proxy::ProxyChild::new(#parent, #expr)
        };}
    }
}

impl quote::ToTokens for ViewToken {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut proxies = HashMap::default();
        self.to_named_tokens(None, 0, tokens, &mut proxies);
        for (proxy, updates) in proxies.into_iter() {
            quote! {
                #proxy.on_update(#updates);
            }
            .to_tokens(tokens);
        }
    }
}

impl ViewToken {
    fn leaf_name(&self) -> &str {
        match self {
            ViewToken::Element { name, .. } => name,
            ViewToken::Text { .. } => "text",
            ViewToken::BlockExpr { .. } => "block_expr",
            ViewToken::BlockProxy { .. } => "block_proxy",
        }
    }

    fn to_named_tokens(
        &self,
        parent_name: Option<syn::Ident>,
        index: usize,
        tokens: &mut proc_macro2::TokenStream,
        proxies: &mut HashMap<syn::Ident, ProxyOnUpdate>,
    ) -> LetIdent {
        let n = if index == 0 {
            String::new()
        } else {
            format!("{index}")
        };

        let spaced_parent_name = parent_name
            .as_ref()
            .map(|name| format!("{}_", name))
            .unwrap_or_default();
        let name = format!("{spaced_parent_name}{}{n}", self.leaf_name());
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
                let (ident, cast) = match ident {
                    None => (
                        quote::format_ident!("_{name}"),
                        Some(syn::parse_str("web_sys::Element").unwrap()),
                    ),
                    Some(LetIdent { ident, cast }) => (ident.clone(), cast.clone()),
                };

                let creation = attributes
                    .iter()
                    .find_map(|att| {
                        if let AttributeToken::Xmlns(ns) = att {
                            Some(WebFlavor::create_element_ns(el, ns))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| WebFlavor::create_element(el));
                quote! {
                    let #ident = #creation;
                }
                .to_tokens(tokens);

                let mut indices = HashMap::<&str, usize>::new();
                for child in children.iter() {
                    let index = indices
                        .entry(child.leaf_name())
                        .and_modify(|i| {
                            *i += 1;
                        })
                        .or_insert(0);
                    let child_id = child
                        .to_named_tokens(Some(ident.clone()), *index, tokens, proxies)
                        .ident;
                    WebFlavor::append_child(&ident, &child_id).to_tokens(tokens);
                }
                for att in attributes.iter() {
                    match att {
                        AttributeToken::Let(outside_id) => {
                            quote! { #outside_id = #ident; }.to_tokens(tokens);
                        }
                        AttributeToken::StyleSingle(key, expr) => {
                            WebFlavor::set_style_property(&ident, key, expr).to_tokens(tokens);
                        }
                        AttributeToken::Attrib(key, expr) => {
                            WebFlavor::set_attribute(&ident, key, expr).to_tokens(tokens);
                        }
                        AttributeToken::On(event, listener) => {
                            WebFlavor::create_listener(&ident, listener, event).to_tokens(tokens);
                        }
                        AttributeToken::Xmlns(_) => {
                            // handled elsewhere
                        }
                        AttributeToken::Window(event, listener) => {
                            WebFlavor::create_window_listener(listener, event).to_tokens(tokens);
                        }
                        AttributeToken::Document(event, listener) => {
                            WebFlavor::create_document_listener(listener, event).to_tokens(tokens);
                        }
                        AttributeToken::AttribProxy(key, proxy_update) => {
                            insert_proxy(
                                proxies,
                                &ident,
                                ProxyUpdateKey::Attrib(key.clone()),
                                proxy_update,
                            );
                            WebFlavor::set_attribute_proxy(&ident, key, proxy_update);
                        }
                    }
                }
                LetIdent { ident, cast }
            }
            ViewToken::Text { ident, expr } => {
                let let_ident = ident.clone().unwrap_or(generic_id);
                let id = let_ident.ident.clone();
                WebFlavor::create_text(&id, expr).to_tokens(tokens);
                let_ident
            }
            ViewToken::BlockExpr { ident, expr } => {
                let let_ident = ident.clone().unwrap_or(generic_id);
                let id = let_ident.ident.clone();
                quote! { let #id = #expr; }.to_tokens(tokens);
                let_ident
            }
            ViewToken::BlockProxy { ident, proxy } => {
                let let_ident = ident.clone().unwrap_or(generic_id);
                let id = let_ident.ident.clone();
                if let Some(parent) = parent_name.as_ref() {
                    insert_proxy(
                        proxies,
                        &id,
                        ProxyUpdateKey::Block {
                            parent: if let Some(parent) = parent_name.as_ref() {
                                parent.clone()
                            } else {
                                syn::Error::new(
                                    proxy.update_ident.span(),
                                    "Cannot use child block pattern for the outer-most block",
                                )
                                .into_compile_error()
                                .to_tokens(tokens);
                                quote::format_ident!("unknown")
                            },
                            block: id.clone(),
                        },
                        proxy,
                    );
                    WebFlavor::proxy_child(parent, &id, proxy).to_tokens(tokens);
                } else {
                    syn::Error::new(
                        proxy.update_ident.span(),
                        "Cannot use child block pattern for the outer-most block",
                    )
                    .into_compile_error()
                    .to_tokens(tokens);
                }
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
    AttribProxy(String, ProxyUpdate),
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
            if keys.len() == 1 && input.fork().parse::<ProxyUpdate>().is_ok() {
                let update = input.parse::<ProxyUpdate>()?;
                Ok(AttributeToken::AttribProxy(keys[0].clone(), update))
            } else {
                let expr = input.parse::<Expr>()?;
                Ok(AttributeToken::from_keys_expr_pair(&keys, expr))
            }
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

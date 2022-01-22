use std::convert::TryFrom;

use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::Error;
use syn_rsx::Node;

use crate::tokens::ViewToken;

pub struct RelayInput {
    pub msg_type: syn::Expr,
    pub name: syn::Ident,
}

impl RelayInput {
    /// The name of this input as it appears in a relay struct field definition.
    pub fn struct_field_name(&self) -> syn::Ident {
        format_ident!("tx_{}", self.name)
    }
}

pub struct RelayOutput {
    pub msg_type: syn::Expr,
    pub name: syn::Ident,
}

impl RelayOutput {
    pub fn struct_field_name(&self) -> syn::Ident {
        format_ident!("{}_chan", self.name)
    }
}

pub enum RelayAttribute {
    Input(RelayInput),
    Output(RelayOutput),
}

impl TryFrom<Node> for RelayAttribute {
    type Error = syn::Error;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        let span = node.name_span().unwrap_or(Span::call_site());
        if let Some(key) = node.name_as_string() {
            if let Some(expr) = node.value {
                match key.split(':').collect::<Vec<_>>().as_slice() {
                    ["input", name] => Ok(RelayAttribute::Input(RelayInput {
                        name: syn::Ident::new(name, span),
                        msg_type: expr,
                    })),
                    ["output", name] => Ok(RelayAttribute::Output(RelayOutput {
                        name: syn::Ident::new(name, span),
                        msg_type: expr,
                    })),
                    _ => Err(Error::new(
                        span,
                        &format!("Unsupported relay attribute: {}", key),
                    )),
                }
            } else {
                Err(Error::new(
                    span,
                    &format!("Relay attribute must have a value: {}", key),
                ))
            }
        } else {
            Err(Error::new(span, "Relay attribute must have a key"))
        }
    }
}

impl RelayAttribute {
    /// Attempt to create a token that defines a single field in a named
    /// relay struct definition.
    pub fn struct_field_def(&self) -> proc_macro2::TokenStream {
        match self {
            RelayAttribute::Input(input) => {
                let tx_input = input.struct_field_name();
                let msg_type = &input.msg_type;
                quote! { #tx_input: mogwai::channel::broadcast::Sender<#msg_type>}
            }
            RelayAttribute::Output(output) => {
                let chan = output.struct_field_name();
                let msg_type = &output.msg_type;
                quote! { #chan: mogwai::channel::broadcast::Sender<#msg_type>}
            }
        }
    }
}

/// ```ignore
/// <MyViewRelay
///  input:set_text = String
///  output:get_click = DomEvent
///  />
/// ```
///
/// ```ignore
/// <MyViewRelay
///  input:set_text = broadcast::Channel<String>
///  output:get_click = broadcast::Channel<DomEvent> >
///
///     <div on:click = get_click.sender()>
///         {("Hello", set_text.receiver())}
///     </div>
///
/// </MyViewRelay>
/// ```
pub struct NamedRelay {
    pub name: String,
    pub attributes: Vec<RelayAttribute>,
    pub views: Vec<ViewToken>,
}

impl TryFrom<Node> for NamedRelay {
    type Error = syn::Error;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        let name_span = node.name_span().unwrap_or(Span::call_site());
        match &node.node_type {
            NodeType::Element => match node.name_as_string() {
                Some(tag) => {
                    let name = tag;

                    let mut attributes = vec![];
                    for attribute in node.attributes.into_iter() {
                        let token = RelayAttribute::try_from(attribute)?;
                        attributes.push(token);
                    }

                    let mut views = vec![];
                    for child in node.children.into_iter() {
                        let token = ViewToken::try_from(child)?;
                        views.push(token);
                    }

                    Ok(NamedRelay {
                        name,
                        attributes,
                        views,
                    })
                }
                None => Err(Error::new(
                    node.name_span().unwrap_or_else(|| Span::call_site()),
                    "Relay is missing a name.",
                )),
            },
            _ => Err(Error::new(
                name_span,
                "The top level node of a relay must be a named element.",
            )),
        }
    }
}

impl NamedRelay {
    /// Produces a struct definition.
    pub fn struct_def(&self) -> proc_macro2::TokenStream {
        let NamedRelay {
            name,
            attributes,
            views,
        } = self;
        let struct_field_defs = attributes.iter().map(RelayAttribute::struct_field_def);

        quote! {
            #[derive(Clone)]
            struct #name<T: mogwai::event::Eventable> {
                inner: mogwai::prelude::Either<mogwai::channel::broadcast::Receiver<T>, T>,
                #(#struct_field_defs,)*
            }
        }
    }

    ///// Produces an implementation definition.
    //pub fn implementation(&self) -> proc_macro2::TokenStream {
    //    let NamedRelay {
    //        name,
    //        attributes,
    //        views,
    //    } = self;

    //    quote! {
    //        impl<T> #name<T>
    //        where
    //            T: mogwai::event::Eventable + mogwai::target::Sendable + Clone + Unpin,
    //            <T as mogwai::event::Eventable>::Event: Clone + mogwai::target::Sendable + Unpin,
    //        {
    //            pub fn new() -> (#name<T>, ViewBuilder<T>) {
    //                use mogwai::futures::IntoSenderSink;

    //                let (tx_t, rx_t) = mogwai::channel::broadcast::bounded::<T>(1);
    //                #(#chans)*

    //                let builder = #builder;

    //                let view = #name {
    //                    inner: mogwai::prelude::Either::Left(rx_t),
    //                    #(#just_field_names,)*
    //                };

    //                (view, builder)
    //            }

    //            pub async fn get_inner(&mut self) -> Result<T, ()> {
    //                use mogwai::futures::StreamExt;
    //                let t = match self.inner.clone() {
    //                    mogwai::prelude::Either::Left(mut rx) => {
    //                        let t = rx.next().await.ok_or(())?;
    //                        self.inner = mogwai::prelude::Either::Right(t.clone());
    //                        t
    //                    }
    //                    mogwai::prelude::Either::Right(t) => t,
    //                };

    //                Ok(t)
    //            }

    //            #(#impl_fns)*
    //        }
    //    }
    //}
}

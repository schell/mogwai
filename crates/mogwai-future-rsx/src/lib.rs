//! RSX for constructing `web-sys` elements.
#![allow(deprecated)]

use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use tokens::{ViewTokenOutput, WebFlavor};

mod tokens;

#[proc_macro]
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match syn::parse::<tokens::ViewToken>(input) {
        Ok(view_token) => ViewTokenOutput::<WebFlavor>::new(&view_token).into_token_stream(),
        Err(error) => error.to_compile_error(),
    }
    .into()
}

/// Derives `ViewChild` for a type.
///
/// The type must contain a field annotated with #[child].
#[proc_macro_derive(ViewChild, attributes(child))]
pub fn impl_derive_viewchild(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: syn::DeriveInput = syn::parse_macro_input!(input);
    let ident = input.ident.clone();
    let all_ty_params = input
        .generics
        .type_params()
        .map(|typ| typ.ident.clone())
        .collect::<Vec<_>>();
    let generics = input
        .generics
        .type_params()
        .map(|p| {
            let mut p = p.clone();
            p.default = None;
            p
        })
        .collect::<Vec<_>>();
    if let syn::Data::Struct(data) = input.data {
        let mut output = quote! {};
        for field in data.fields.iter() {
            let has_child_annotation = field.attrs.iter().any(|attr| attr.path().is_ident("child"));
            if has_child_annotation {
                let ty = &field.ty;
                let field = &field.ident;
                output = quote! {
                    impl <#(#generics),*> mogwai_futura::prelude::ViewChild for #ident<#(#all_ty_params),*> {
                        type Node = <#ty as mogwai_futura::prelude::ViewChild>::Node;

                        fn as_append_arg(&self) -> mogwai_futura::prelude::AppendArg<impl Iterator<Item = Self::Node>> {
                            self.#field.as_append_arg()
                        }
                    }
                };
                break;
            }
        }
        output
    } else {
        quote! { compile_error!("Deriving ViewChild is only supported on struct types") }
    }
    .into()
}

struct FromAnnotationArgs {
    pattern: syn::Pat,
    code: syn::Expr,
}

impl syn::parse::Parse for FromAnnotationArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let pattern = syn::Pat::parse_single(input)?;
        let _ = input.parse::<syn::Token![=]>()?;
        let _ = input.parse::<syn::Token![>]>()?;
        let code = input.parse::<syn::Expr>()?;
        Ok(FromAnnotationArgs { pattern, code })
    }
}

/// Derives `From<T<Builder>` for a type.
///
/// The type must contain a type variable constrained by `View`.
///
/// ```rust
/// #[derive(FromBuilder)]
/// struct MyView<V: View> {
///     wrapper: V::Element<web_sys::Element>,
///     #[from(from_rows)]
///     rows: Vec<V::Element<web_sys::Element>,
/// }
///
/// fn from_rows(rows: Vec<ElementBuilder>) -> Vec<web_sys::Element> {
///     rows.into_iter().map(|builder| builder.into()).collect()
/// }
/// ```
#[proc_macro_derive(FromBuilder, attributes(from))]
pub fn impl_derive_from(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: syn::DeriveInput = syn::parse_macro_input!(input);
    let ident = input.ident.clone();
    let (all_ty_params, maybe_view_ty_param) =
        input
            .generics
            .type_params()
            .fold((vec![], None), |(mut all, mut found), typ| {
                all.push(typ.ident.clone());

                for bound in typ.bounds.iter() {
                    if let syn::TypeParamBound::Trait(t) = bound {
                        if let Some(last) = t.path.segments.last() {
                            if last.ident == "View" {
                                found = Some(typ.ident.clone());
                            }
                        }
                    }
                }

                (all, found)
            });
    let view_ty_param = if let Some(p) = maybe_view_ty_param {
        p
    } else {
        return syn::Error::new(
            input.generics.span(),
            "Type must contain a type parameter constrained by View",
        )
        .into_compile_error()
        .into();
    };
    let ty_params_minus_view = all_ty_params
        .iter()
        .filter(|p| *p != &view_ty_param)
        .collect::<Vec<_>>();
    let ty_params_view_replaced_with_web = all_ty_params
        .iter()
        .map(|p| {
            if *p == view_ty_param {
                quote! { mogwai_futura::web::Web }
            } else {
                quote! { #p }
            }
        })
        .collect::<Vec<_>>();
    if let syn::Data::Struct(data) = input.data {
        let mut output = vec![];
        for field in data.fields.iter() {
            let maybe_from_attr = field.attrs.iter().find(|attr| attr.path().is_ident("from"));
            let id = field.ident.as_ref().unwrap();
            if let Some(attr) = maybe_from_attr {
                let args: FromAnnotationArgs = match attr.parse_args() {
                    Ok(args) => args,
                    Err(e) => return e.to_compile_error().into(),
                };
                let pattern = args.pattern;
                let code = args.code;
                output.push(quote! {
                    #id: {
                        let #pattern = value.#id;
                        #code
                    }
                })
            } else {
                output.push(quote! {
                    #id: value.#id.into()
                });
            }
        }
        let from_ty = quote! { #ident<#(#ty_params_minus_view),*> };
        let to_ty = quote! { #ident<#(#ty_params_view_replaced_with_web),*> };
        quote! {
            impl <#(#ty_params_minus_view),*> From<#from_ty> for #to_ty {
                fn from(value: #ident) -> Self {
                    Self {
                        #(#output),*
                    }
                }
            }
        }
    } else {
        quote! { compile_error!("Deriving ViewChild is only supported on struct types") }
    }
    .into()
}

#[cfg(test)]
mod ssr_tests {
    use std::str::FromStr;

    #[test]
    fn can_parse_rust_closure() {
        let expr: syn::Expr = syn::parse_str(r#"|i:i32| format!("{}", i)"#).unwrap();
        match expr {
            syn::Expr::Closure(_) => {}
            _ => panic!("wrong expr parse, expected closure"),
        }
    }

    #[test]
    fn can_token_stream_from_string() {
        let _ts = proc_macro2::TokenStream::from_str(r#"|i:i32| format!("{}", i)"#).unwrap();
    }

    #[test]
    fn can_parse_from_token_stream() {
        let _ts = proc_macro2::TokenStream::from_str(r#"<div class="any_class" />"#).unwrap();
    }
}

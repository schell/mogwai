//! RSX for constructing `web-sys` elements.
#![allow(deprecated)]

use quote::{ToTokens, quote};
use syn::spanned::Spanned;

mod tokens;

#[proc_macro]
/// View construction macro.
///
/// The `rsx!` macro facilitates the creation of UI components using a syntax
/// similar to JSX, allowing for a more intuitive and declarative way to define
/// views in Rust.
///
/// This macro transforms a tree of HTML-like syntax into Rust code that constructs
/// the corresponding UI elements. It supports embedding Rust expressions and
/// handling events, making it a powerful tool for building dynamic interfaces.
///
/// # Example
///
/// ```rust
/// rsx! {
///     let root = div(class = "container") {
///         h1 { "Hello, World!" }
///         button(on:click = handle_click) { "Click me" }
///     }
/// }
/// ```
///
/// In this example, `rsx!` is used to create a `div` with a class and two child
/// elements: an `h1` and a `button` with an event listener `handle_click`.
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match syn::parse::<tokens::ViewToken>(input) {
        Ok(view_token) => view_token.into_token_stream(),
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
                let field = &field.ident;
                output = quote! {
                    impl <#(#generics),*> mogwai::prelude::ViewChild<#view_ty_param> for #ident<#(#all_ty_params),*> {
                        fn as_append_arg(&self) -> mogwai::prelude::AppendArg<#view_ty_param, impl Iterator<Item = std::borrow::Cow<'_, #view_ty_param::Node>>> {
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

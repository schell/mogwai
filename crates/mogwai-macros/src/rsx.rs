//! Support for html style and function-style RSX.

use std::convert::TryFrom;
use quote::quote;

use crate::{
    combine_errors,
    tokens::{ViewToken, ViewTokens},
};

/// Parse an html-style RSX macro.
pub fn parse_html(
    input: proc_macro::TokenStream,
) -> Result<(Vec<ViewToken>, Vec<syn::Error>), syn::Error> {
    syn_rsx::parse(input)
        .map(|parsed| crate::partition_unzip(parsed.into_iter(), ViewToken::try_from))
}

/// Parse a function-style RSX macro.
pub fn parse_fn(
    input: proc_macro::TokenStream,
) -> Result<(Vec<ViewToken>, Vec<syn::Error>), syn::Error> {
    let tokens: ViewTokens = syn::parse(input)?;
    Ok((tokens.views, vec![]))
}

/// Parse either a function-style macro or an html-style macro,
/// in that order.
pub fn parse_with(
    input: proc_macro::TokenStream,
    f: impl FnOnce(proc_macro::TokenStream) -> Result<(Vec<ViewToken>, Vec<syn::Error>), syn::Error>,
) -> proc_macro::TokenStream {
    let tokens = match f(input) {
        Ok((view_tokens, errs)) => {
            if let Some(error) = combine_errors(errs) {
                return error.to_compile_error().into();
            }
            let (tokens, errs) =
                crate::partition_unzip(view_tokens.iter(), crate::node_to_builder_token_stream);
            if let Some(error) = combine_errors(errs) {
                return error.to_compile_error().into();
            }

            match tokens.len() {
                0 => quote! { compile_error("builder! macro must not be empty") },
                1 => {
                    let ts = &tokens[0];
                    quote! { #ts }
                }
                _ => quote! { vec![#(#tokens),*] },
            }
        }
        Err(error) => error.to_compile_error(),
    };

    proc_macro::TokenStream::from(tokens)
}

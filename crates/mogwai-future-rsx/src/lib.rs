//! RSX for constructing `web-sys` elements.
#![allow(deprecated)]

use quote::ToTokens;
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

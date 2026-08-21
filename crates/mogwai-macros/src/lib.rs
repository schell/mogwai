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
/// This macro transforms a tree of HTML-like syntax into Rust code that
/// constructs the corresponding UI elements. It supports `let` binding,
/// embedding Rust expressions, and handling events, making it a powerful tool
/// for building dynamic interfaces.
///
/// # Quick Reference
///
/// ```rust
/// use mogwai::prelude::*;
///
/// fn view<V: View>() -> V::Element {
///     rsx! {
///         let root = div(class = "container") {
///             h1() { "Hello, World!" }
///             button(on:click = handle_click) { "Click me" }
///         }
///     }
///     root
/// }
/// ```
///
/// The `let` binding is the macro's "return value": `root` is in scope after
/// the macro expands. A type parameter `V: View` must be in scope.
///
/// # Special Attributes
///
/// - **`on:event = name`**: Registers an event listener on the element and
///   binds it to `name`. The name is the **output** variable, not a
///   pre-existing handler. Expansion: `let name = element.listen("event");`.
/// - **`window:event = name`**: Creates a window-level event listener.
///   Expansion: `let name = V::EventListener::on_window("event");`.
/// - **`document:event = name`**: Creates a document-level event listener.
///   Expansion: `let name = V::EventListener::on_document("event");`.
/// - **`style:name = expr`**: Sets a single inline-style property via
///   `set_style`. The name undergoes underscore-to-dash conversion
///   (e.g., `style:background_color` -> `background-color`).
/// - **`style = "a: b; c: d;"`**: Sets the full `style` attribute string via
///   `set_property("style", ...)`.
/// - **`xmlns = expr`**: Triggers element creation via `new_namespace` for
///   SVG and other namespaced XML.
///
/// # Underscore-to-Dash Conversion
///
/// Attribute names undergo underscore-to-dash conversion. Leading and
/// trailing underscores are stripped, interior underscores become dashes:
///
/// | RSX name | HTML attribute |
/// |----------|----------------|
/// | `type_` | `type` |
/// | `for_` | `for` |
/// | `aria_hidden` | `aria-hidden` |
/// | `stroke_width` | `stroke-width` |
///
/// The `type` keyword is also special-cased by the parser, so `type =
/// "checkbox"` (without underscore) works too.
///
/// # Proxy: Reactive Updates
///
/// The `Proxy<T>` type provides reactive updates. The macro has special syntax
/// in both attribute and node positions:
///
/// ```rust
/// use mogwai::ssr::prelude::*;
///
/// #[derive(Debug, PartialEq)]
/// struct Status {
///     color: String,
///     message: String,
/// }
///
/// struct Widget<V: View> {
///     root: V::Element,
///     state: Proxy<Status>,
/// }
///
/// fn new_widget<V: View>() -> Widget<V> {
///     let mut state = Proxy::new(Status {
///         color: "black".to_string(),
///         message: "Hello".to_string(),
///     });
///
///     rsx! {
///         let root = div() {
///             p(
///                 id = "message_wrapper",
///                 style:color = state(s => &s.color)
///             ) {
///                 {state(s => &s.message)}
///             }
///         }
///     }
///
///     Widget { root, state }
/// }
///
/// let mut w = new_widget::<mogwai::ssr::Ssr>();
/// assert_eq!(
///     r#"<div><p id="message_wrapper" style="color: black;">Hello</p></div>"#,
///     w.root.html_string()
/// );
///
/// w.state.set(Status {
///     color: "red".to_string(),
///     message: "Goodbye".to_string(),
/// });
/// assert_eq!(
///     r#"<div><p id="message_wrapper" style="color: red;">Goodbye</p></div>"#,
///     w.root.html_string()
/// );
/// ```
///
/// # Nesting Components via `ViewChild`
///
/// Any type implementing `ViewChild` can be used as a child node. Derive
/// `ViewChild` with `#[derive(ViewChild)]` on a struct with a `#[child]`
/// field:
///
/// ```rust
/// use mogwai::prelude::*;
///
/// #[derive(ViewChild)]
/// struct MyComponent<V: View> {
///     #[child]
///     wrapper: V::Element,
/// }
///
/// fn create_view<V: View>() -> V::Element {
///     rsx! {
///         let wrapper = div() {
///             "This is a custom component."
///         }
///     }
///
///     let component = MyComponent::<V> { wrapper };
///
///     rsx! {
///         let root = div() {
///             h1() { "Welcome" }
///             {component}
///         }
///     }
///
///     root
/// }
/// ```
///
/// # Full Documentation
///
/// For the complete syntax reference, attribute table, gotchas, and advanced
/// patterns, see the `mogwai::rsx` module documentation in the `mogwai`
/// crate.
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match syn::parse::<tokens::ViewToken>(input) {
        Ok(view_token) => view_token.into_token_stream(),
        Err(error) => error.to_compile_error(),
    }
    .into()
}

/// Derives `ViewChild` for a type.
///
/// The type must contain a field annotated with `#[child]`.
///
/// Deriving `ViewChild` for an arbitrary Rust type allows you to use that type
/// in the node position of an [`rsx!`] macro.
///
/// # Example
///
/// ```rust
/// use mogwai::prelude::*;
///
/// #[derive(ViewChild)]
/// struct MyComponent<V: View> {
///     #[child]
///     wrapper: V::Element,
/// }
///
/// fn nest<V: View>(component: &MyComponent<V>) -> V::Element {
///     rsx! {
///         let wrapper = div() {
///             h1(){ "Hello, world!" }
///             {component} // <- here `component` is added to the view tree
///         }
///     }
///
///     wrapper
/// }
/// ```
///
/// In this example, `MyComponent` is a struct that derives `ViewChild`,
/// allowing it to be used within the `rsx!` macro. The `wrapper` field is
/// annotated with `#[child]`, indicating that it is the primary child node for
/// the component.
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
                    if let syn::TypeParamBound::Trait(t) = bound
                        && let Some(last) = t.path.segments.last()
                        && last.ident == "View"
                    {
                        found = Some(typ.ident.clone());
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

/// Derives `ViewProperties` for a type.
///
/// The type must contain a field annotated with `#[properties]`. All
/// `ViewProperties` trait methods will be proxied to that field.
///
/// This is useful for wrapper/component types that contain a view element and
/// want to expose property and style manipulation without manual delegation.
///
/// # Example
///
/// ```rust
/// use mogwai::prelude::*;
///
/// #[derive(ViewProperties)]
/// struct MyComponent<V: View> {
///     #[properties]
///     wrapper: V::Element,
/// }
///
/// fn set_attrs<V: View>(component: &MyComponent<V>) {
///     component.set_property("class", "active");
///     component.set_style("color", "red");
///     assert!(component.has_property("class"));
/// }
/// ```
///
/// In this example, `MyComponent` derives `ViewProperties`, allowing property
/// and style methods to be called directly on the component. The `wrapper`
/// field is annotated with `#[properties]`, indicating that it is the
/// underlying element to which all property operations are delegated.
#[proc_macro_derive(ViewProperties, attributes(properties))]
pub fn impl_derive_view_properties(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: syn::DeriveInput = syn::parse_macro_input!(input);
    let ident = input.ident.clone();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    if let syn::Data::Struct(data) = input.data {
        for field in data.fields.iter() {
            let has_properties_annotation = field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("properties"));
            if has_properties_annotation {
                let field_ident = &field.ident;
                return quote! {
                    impl #impl_generics mogwai::prelude::ViewProperties for #ident #ty_generics #where_clause {
                        fn has_property(&self, property: impl AsRef<str>) -> bool {
                            self.#field_ident.has_property(property)
                        }

                        fn get_property(&self, property: impl AsRef<str>) -> Option<mogwai::prelude::Str> {
                            self.#field_ident.get_property(property)
                        }

                        fn set_property(&self, property: impl AsRef<str>, value: impl AsRef<str>) {
                            self.#field_ident.set_property(property, value)
                        }

                        fn remove_property(&self, property: impl AsRef<str>) {
                            self.#field_ident.remove_property(property)
                        }

                        fn set_style(&self, key: impl AsRef<str>, value: impl AsRef<str>) {
                            self.#field_ident.set_style(key, value)
                        }

                        fn remove_style(&self, key: impl AsRef<str>) {
                            self.#field_ident.remove_style(key)
                        }
                    }
                }
                .into();
            }
        }
        syn::Error::new(
            ident.span(),
            "A field annotated with `#[properties]` is required when deriving ViewProperties",
        )
        .into_compile_error()
        .into()
    } else {
        quote! { compile_error!("Deriving ViewProperties is only supported on struct types") }
            .into()
    }
}


#[proc_macro]
pub fn impl_parameters_tuples(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let tuple: syn::TypeTuple = syn::parse_macro_input!(input);
    let tys = tuple.elems.iter().collect::<Vec<_>>();
    let params = tys
        .iter()
        .enumerate()
        .map(|(i, ty)| syn::Ident::new(&format!("__{i}"), ty.span()))
        .collect::<Vec<_>>();
    let output = quote! {
        impl<#(#tys),*> mogwai::web::event::Parameters for #tuple
        where
            #(#tys: wasm_bindgen::convert::FromWasmAbi + 'static),*,
        {
            type Closure = Closure<dyn FnMut(#(#tys),*)>;

            fn into_arity_closure(mut f: Box<dyn FnMut(Self)>) -> Self::Closure {
                Closure::wrap(Box::new(move |#(#params),*| {
                    f((#(#params),*));
                }))
            }
        }
    };
    output.into()
}

#[cfg(test)]
mod test {
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

    #[test]
    #[allow(dead_code)]
    fn moggy() {
        use mogwai::prelude::*;

        #[derive(ViewChild)]
        struct MyComponent<V: View> {
            #[child]
            wrapper: V::Element,
        }

        fn create_view<V: View>() -> V::Element {
            rsx! {
                let wrapper = div() {
                    "This is a custom component."
                }
            }
            let component = MyComponent::<V> { wrapper };

            rsx! {
                let root = div() {
                    h1() { "Welcome" }
                    {component} // Using the custom component within the view
                }
            }

            root
        }
    }

    #[test]
    #[allow(dead_code)]
    fn nest() {
        use mogwai::prelude::*;

        #[derive(ViewChild)]
        struct MyComponent<V: View> {
            #[child]
            wrapper: V::Element,
        }

        fn nest<V: View>(component: &MyComponent<V>) -> V::Element {
            rsx! {
                let wrapper = div() {
                    h1(){ "Hello, world!" }
                    {component} // <- here `component` is added to the view tree
                }
            }

            wrapper
        }
    }

    #[test]
    #[allow(dead_code)]
    fn nest_with_block() {
        use mogwai::prelude::*;

        #[derive(ViewChild)]
        struct MyComponent<V: View> {
            #[child]
            wrapper: V::Element,
            text: V::Text,
        }

        impl<V: View> MyComponent<V> {
            fn new() -> Self {
                rsx! {
                    let wrapper = p() {
                        let text = "Here is text"
                    }
                }
                Self { wrapper, text }
            }
        }

        fn nest<V: View>() -> V::Element {
            rsx! {
                let wrapper = div() {
                    h1(){ "Hello, world!" }
                    {{
                        let component = MyComponent::<V>::new();
                        component.text.set_text("blarg");
                        component
                    }}
                }
            }

            wrapper
        }
    }

    #[test]
    fn derive_view_properties() {
        use mogwai::prelude::*;

        #[derive(ViewProperties)]
        struct MyComponent<V: View> {
            #[properties]
            wrapper: V::Element,
        }

        fn set_attrs<V: View>(component: &MyComponent<V>) {
            component.set_property("class", "active");
            component.set_style("color", "red");
            assert!(component.has_property("class"));
        }

        let el = mogwai::ssr::SsrElement::new("div");
        let component = MyComponent::<mogwai::ssr::Ssr> { wrapper: el };
        set_attrs(&component);
        assert_eq!(component.get_property("class"), Some("active".into()));
    }

    #[test]
    fn derive_view_child_and_view_properties() {
        use mogwai::prelude::*;

        #[derive(ViewChild, ViewProperties)]
        struct MyComponent<V: View> {
            #[child]
            #[properties]
            wrapper: V::Element,
            _text: V::Text,
        }

        fn use_component<V: View>(component: &MyComponent<V>) -> V::Element {
            component.set_property("id", "my-component");
            rsx! {
                let root = div() {
                    {component}
                }
            }
            root
        }

        let wrapper = mogwai::ssr::SsrElement::new("span");
        let text = mogwai::ssr::SsrText::new("hello");
        let component = MyComponent::<mogwai::ssr::Ssr> {
            wrapper,
            _text: text,
        };
        let _root = use_component(&component);
        assert!(component.has_property("id"));
        assert_eq!(component.get_property("id"), Some("my-component".into()));
    }

    #[test]
    fn derive_view_properties_separate_fields() {
        use mogwai::prelude::*;

        #[derive(ViewChild, ViewProperties)]
        struct MyComponent<V: View> {
            #[child]
            root: V::Element,
            #[properties]
            inner: V::Element,
        }

        let root = mogwai::ssr::SsrElement::new("div");
        let inner = mogwai::ssr::SsrElement::new("span");
        let component = MyComponent::<mogwai::ssr::Ssr> { root, inner };
        component.set_property("class", "inner-class");
        // The property is on `inner`, not `root`
        assert!(!component.root.has_property("class"));
        assert!(component.inner.has_property("class"));
    }
}

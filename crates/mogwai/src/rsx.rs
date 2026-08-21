//! # RSX: View Construction Macro
//!
//! The [`rsx!`](crate::rsx) macro is mogwai's tool for declaratively building UI
//! views using a syntax similar to JSX. It transforms a tree of HTML-like
//! elements, attributes, text nodes, and Rust expressions into Rust code that
//! constructs the corresponding platform-agnostic view elements.
//!
//! Unlike many template macros, `rsx!` does not "return" a value in the
//! traditional sense. Instead, it expands to a series of `let` statements. The
//! variable you bind with `let` at the outermost level **is** the result of the
//! macro and is available in the surrounding scope. This is the mechanism by
//! which the built view escapes the macro.
//!
//! `rsx!` requires a type parameter `V: `[`View`] in the enclosing scope. The
//! macro expands to calls on `V`'s associated types (`V::Element`, `V::Text`,
//! `V::EventListener`), so the function or `impl` block containing the macro
//! must be generic over `V: View`.
//!
//! For a tutorial introduction, see [`crate::an_introduction`]. This module is
//! the comprehensive syntax reference.
//!
//! # Table of Contents
//!
//! - [Quick Example](#quick-example)
//! - [Syntax Grammar](#syntax-grammar)
//! - [Elements](#elements)
//! - [Text Nodes](#text-nodes)
//! - [`let` Bindings](#let-bindings)
//! - [Scoping](#scoping)
//! - [Attributes](#attributes)
//! - [Attribute Reference Table](#attribute-reference-table)
//! - [Event Listeners](#event-listeners)
//! - [Inline Styles](#inline-styles)
//! - [XML Namespaces and SVG](#xml-namespaces-and-svg)
//! - [Block Expressions](#block-expressions)
//! - [Proxy: Reactive Updates](#proxy-reactive-updates)
//!   - [Proxy in Attribute Position](#proxy-in-attribute-position)
//!   - [Proxy in Node Position](#proxy-in-node-position)
//!   - [Proxy Semantics](#proxy-semantics)
//! - [Nesting Components](#nesting-components)
//! - [Lists and Conditionals](#lists-and-conditionals)
//! - [Cross-Platform Views](#cross-platform-views)
//! - [Gotchas and Edge Cases](#gotchas-and-edge-cases)
//!
//! # Quick Example
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct Widget<V: View> {
//!     root: V::Element,
//! }
//!
//! impl<V: View> Widget<V> {
//!     fn new() -> Self {
//!         rsx! {
//!             let root = div(class = "container") {
//!                 h1() { "Hello, World!" }
//!                 button(on:click = handle_click) { "Click me" }
//!             }
//!         }
//!         Self { root }
//!     }
//! }
//! ```
//!
//! Here `root` is bound by the outermost `let` and is available after the
//! macro expands. The `handle_click` name is created by the macro (see
//! [Event Listeners](#event-listeners)).
//!
//! # Syntax Grammar
//!
//! The macro parses a single root [`ViewToken`], which is one of four node
//! kinds:
//!
//! ```text
//! ViewToken ::=
//!     Element
//!   | Text
//!   | BlockExpr
//!   | BlockProxy
//!
//! Element ::=
//!     [let ident [: Type] =] tag_name [( Attribute, ... )]
//!           { ViewToken ... }
//!
//! Text ::=
//!     [let ident [: Type] =] "string literal"
//!
//! BlockExpr ::=
//!     [let ident [: Type] =] { rust_expression }
//!
//! BlockProxy ::=
//!     [let ident [: Type] =] { proxy_ident ( pattern => expression ) }
//!
//! Attribute ::=
//!     key_segment [: key_segment ...] [= expression | ProxyUpdate]
//!   | ident          -- bare alias (attribute-position `let`)
//! ```
//!
//! Each node kind is described in detail below.
//!
//! # Elements
//!
//! An element is written as `tag_name(attributes...) { children... }`. The tag
//! name is any valid Rust identifier and is passed as a string to
//! [`ViewElement::new`]. Tag names are **not validated at compile time};
//! `rsx! { foobar() {} }` compiles fine and only fails at runtime if the
//! platform rejects the tag.
//!
//! The attribute parentheses are optional: `div { ... }` is equivalent to
//! `div() { ... }`. The brace body is **required** (elements must have a
//! braced body, even if empty).
//!
//! ## Empty elements
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     rsx! {
//!         let root = div() {
//!             br(){}
//!             input(type_ = "text", placeholder = "Type here") {}
//!             hr(){}
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! ## Sibling elements
//!
//! Multiple children of the same type are fine. The macro auto-generates
//! unique internal names for unbound nodes, so there are no naming conflicts:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     rsx! {
//!         let root = div() {
//!             "Text one."
//!             "Text two."
//!             "Text three."
//!             p() { "Inside p one." }
//!             p() { "Inside p two." }
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! # Text Nodes
//!
//! String literals become [`ViewText::new`] calls, producing `V::Text` nodes:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     rsx! {
//!         let root = p() {
//!             "Hello, World!"
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! ## Dynamic text via `into_text`
//!
//! For text produced by Rust expressions inside a `{ }` block (not a string
//! literal), use the [`ViewTextExt::into_text`] extension method to convert
//! any `impl AsRef<str>` into `V::Text`:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     let name = "World";
//!     rsx! {
//!         let root = p() {
//!             {format!("Hello, {name}!").into_text::<V>()}
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! String literals in node position do **not** need `into_text`; the macro
//! handles them directly. Only block expressions that produce text require it.
//!
//! # `let` Bindings
//!
//! The `let` binding is the central mechanism of `rsx!`. It works in two
//! positions:
//!
//! ## Root `let` binding (the return value)
//!
//! The outermost node can be prefixed with `let ident [: Type] =` to bind the
//! constructed view to a variable. This variable is in scope after the macro
//! and is how the view "escapes":
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     rsx! {
//!         let root = div(class = "my-div") {
//!             "content"
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! An optional type annotation (`let root: V::Element = ...`) is parsed but
//! not applied as a cast. The bound type is determined by the node kind
//! (element, text, etc.).
//!
//! ## Nested `let` bindings (capturing inner nodes)
//!
//! Any child node can also be prefixed with `let` to capture that node into a
//! local variable for later use. This is the primary way to extract handles to
//! inner elements, text nodes, and event listeners:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct MyView<V: View> {
//!     root: V::Element,
//!     button: V::Element,
//!     label: V::Text,
//! }
//!
//! impl<V: View> MyView<V> {
//!     fn new() -> Self {
//!         rsx! {
//!             let root = div() {
//!                 let button = button() {
//!                     let label = "Click me"
//!                 }
//!             }
//!         }
//!         Self { root, button, label }
//!     }
//! }
//! ```
//!
//! ## Auto-generated names
//!
//! If you omit `let` on a child node, the macro generates a unique name based
//! on the parent name, the leaf type, and an index. For example, a `div` with
//! three `p` children produces names like `_div_p`, `_div_p1`, `_div_p2`.
//! These are internal and not visible to your code, but they ensure there are
//! no naming conflicts among anonymous siblings.
//!
//! ## Attribute-position `let` aliasing
//!
//! A bare identifier (no `=`) in the attribute list aliases the element to
//! that name. This is equivalent to using `let` on the element, but is written
//! inside the parentheses. For example, `div(my_ref, class = "x") { ... }`
//! assigns the constructed `div` to `my_ref` in the surrounding scope.
//!
//! # Scoping
//!
//! The braces in `rsx!` element syntax are **syntactic delimiters, not Rust
//! block scopes**. They express the view tree to the parser and provide
//! structure for tooling (syntax highlighting, formatters, language servers),
//! but the macro does not create a new Rust scope for each `{ }`. Instead, all
//! `let` bindings from the entire tree are emitted as a single flat sequence
//! of statements into the **enclosing scope**.
//!
//! ## Why the bindings flatten
//!
//! This is a deliberate ergonomic choice. A `rsx!` block typically lives
//! inside `fn new()` or `Default::default()`, and its job is to construct a
//! view and capture every node the owning struct needs. By flattening all
//! bindings into one scope, a deeply nested `let` is directly available at the
//! top level to populate struct fields, without threading values back out
//! through closures or return tuples:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct MyView<V: View> {
//!     root: V::Element,
//!     completed_input: V::Element,
//! }
//!
//! impl<V: View> MyView<V> {
//!     fn new() -> Self {
//!         rsx! {
//!             let root = section() {
//!                 header() {
//!                     div() {
//!                         let completed_input = input(type_ = "checkbox") {}
//!                     }
//!                 }
//!             }
//!         }
//!         // `completed_input` is in scope here, alongside `root`, even though
//!         // it was bound three levels deep in the tree.
//!         Self { root, completed_input }
//!     }
//! }
//! ```
//!
//! If `rsx!` introduced real block scopes per element, you'd have to extract
//! each captured node from its nested scope, which would fight against the
//! natural tree structure of the markup.
//!
//! ## Consequences
//!
//! Because all bindings share one scope:
//!
//! - **Names must be unique across the entire macro invocation.** Sibling
//!   subtrees cannot reuse the same `let` name. This is why the macro
//!   auto-generates unique names (`_div_p1`, `_div_p2`) for unbound nodes.
//! - **Any captured node is accessible after the macro**, regardless of
//!   nesting depth. The order of `let` statements in the flattened output
//!   follows a pre-order traversal (parent created before children, children
//!   appended after parent), so parents are always initialized before their
//!   children.
//! - **The braces are for the parser and tooling**, not for scope. Syntax
//!   highlighters, formatters, and language servers can use the brace
//!   structure to understand the tree; Rust's scoping rules do not apply
//!   within `rsx!`.
//!
//! ## Capturing a deeply nested node
//!
//! A common pattern is to capture a container element deep in the tree for
//! later imperative mutation (adding/removing children at runtime):
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! #[derive(ViewChild)]
//! struct ItemSet<V: View> {
//!     #[child]
//!     fieldset: V::Element,
//!     ol: V::Element,
//! }
//!
//! impl<V: View> Default for ItemSet<V> {
//!     fn default() -> Self {
//!         rsx! {
//!             let fieldset = fieldset() {
//!                 legend(){ "Items" }
//!                 let ol = ol(){ }
//!             }
//!         }
//!         // Both `fieldset` (the root) and `ol` (a deeply nested child)
//!         // are in scope here, ready to populate the struct.
//!         Self { fieldset, ol }
//!     }
//! }
//! ```
//!
//! # Attributes
//!
//! Attributes appear in the parenthesized group after the tag name,
//! comma-separated. Each attribute is a sequence of colon-separated key
//! segments optionally followed by `= expression`.
//!
//! ## Static attributes
//!
//! The right-hand side of `=` is any Rust expression evaluating to
//! `impl AsRef<str>`:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     let href = "http://zyghost.com";
//!     rsx! {
//!         let root = a(href = href, class = "link") {
//!             "Schellsan's website"
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! ## Underscore-to-dash conversion
//!
//! Attribute names undergo underscore-to-dash conversion. Leading and trailing
//! underscores are stripped, and interior underscores become dashes:
//!
//! | RSX name | HTML attribute |
//! |----------|----------------|
//! | `class` | `class` |
//! | `aria_hidden` | `aria-hidden` |
//! | `stroke_width` | `stroke-width` |
//! | `type_` | `type` |
//! | `for_` | `for` |
//!
//! This allows you to write valid Rust identifiers for HTML attributes that
//! contain hyphens (which are not valid in Rust identifiers).
//!
//! ## Reserved-word attributes
//!
//! The `type` keyword is special-cased by the parser: you can write either
//! `type = "checkbox"` (using the `type` keyword directly) or `type_ =
//! "checkbox"` (which goes through underscore-to-dash conversion). Both
//! produce the `type` attribute. The `type_` form is preferred for clarity.
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     rsx! {
//!         let root = input(type_ = "checkbox", class = "toggle") {}
//!     }
//!     root
//! }
//! ```
//!
//! ## Multi-segment keys
//!
//! Colon-separated key segments are joined with `:` after underscore-to-dash
//! conversion. Single-segment keys become regular attributes. The first
//! segment is checked against a set of special prefixes (see the
//! [Attribute Reference Table](#attribute-reference-table)). Unknown
//! multi-segment keys are joined with `:` and treated as regular attributes.
//!
//! ## Boolean / valueless attributes
//!
//! If an attribute has multiple colon-separated key segments and no `=`,
//! it becomes a regular attribute with a value of `None`. This is an edge
//! case and not commonly used.
//!
//! # Attribute Reference Table
//!
//! | Prefix | Syntax | Expansion | Description |
//! |--------|--------|-----------|-------------|
//! | *(none)* | `name = expr` | `element.set_property("name", expr)` | Regular HTML attribute |
//! | `on:` | `on:event = name` | `let name = element.listen("event")` | Element event listener (see [Event Listeners](#event-listeners)) |
//! | `window:` | `window:event = name` | `let name = V::EventListener::on_window("event")` | Window-level event listener |
//! | `document:` | `document:event = name` | `let name = V::EventListener::on_document("event")` | Document-level event listener |
//! | `style:` | `style:name = expr` | `element.set_style("name", expr)` | Single inline-style property |
//! | *(none)* | `style = "a: b; c: d;"` | `element.set_property("style", expr)` | Full inline-style string |
//! | `xmlns` | `xmlns = expr` | `V::Element::new_namespace(tag, expr)` | XML namespace (see [SVG](#xml-namespaces-and-svg)) |
//! | *(proxy)* | `name = proxy(pat => expr)` | immediate `set_property` + `on_update` | Reactive attribute (see [Proxy](#proxy-in-attribute-position)) |
//! | *(proxy)* | `style:name = proxy(pat => expr)` | immediate `set_style` + `on_update` | Reactive style |
//!
//! # Event Listeners
//!
//! ## Element events: `on:event = name`
//!
//! Attaches an event listener to the element. **The name on the right-hand
//! side is the output variable, not a pre-existing handler.** The macro
//! expands to `let name = element.listen("event");`, creating a
//! [`ViewEventListener`] and binding it to `name`. You then await events on
//! `name` in async logic.
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct Button<V: View> {
//!     root: V::Element,
//!     on_click: V::EventListener,
//! }
//!
//! impl<V: View> Button<V> {
//!     fn new() -> Self {
//!         rsx! {
//!             let root = button(on:click = on_click) {
//!                 "Click me"
//!             }
//!         }
//!         Self { root, on_click }
//!     }
//!
//!     async fn wait_for_click(&self) {
//!         let _ev: V::Event = self.on_click.next().await;
//!     }
//! }
//! ```
//!
//! This is the most important gotcha: `on:click = on_click` **creates** a
//! listener and **binds** it to `on_click`. It does not attach a function
//! called `on_click`. The pattern is "register, then bind, then await".
//!
//! The event name after `on:` is any identifier; there is no fixed list.
//! Common events: `click`, `change`, `dblclick`, `blur`, `keyup`, `input`,
//! `submit`, `mouseover`, `focus`.
//!
//! ## Window events: `window:event = name`
//!
//! Creates a listener on the global window object, independent of the element
//! it is written on. The element is irrelevant:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> (V::Element, V::EventListener) {
//!     rsx! {
//!         let root = div(window:resize = on_resize) {
//!             "Resize the window to see events"
//!         }
//!     }
//!     (root, on_resize)
//! }
//! ```
//!
//! ## Document events: `document:event = name`
//!
//! Creates a listener on the global document object:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> (V::Element, V::EventListener) {
//!     rsx! {
//!         let root = div(document:keydown = on_keydown) {
//!             "Press any key"
//!         }
//!     }
//!     (root, on_keydown)
//! }
//! ```
//!
//! # Inline Styles
//!
//! ## `style:` shorthand
//!
//! `style:name = expr` sets a single CSS property via
//! [`ViewProperties::set_style`]. The name undergoes underscore-to-dash
//! conversion, so `style:background_color` becomes `background-color`:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     rsx! {
//!         let root = div(style:cursor = "pointer", style:background_color = "blue") {
//!             "Styled div"
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! ## Full `style` string
//!
//! `style = "a: b; c: d;"` sets the entire `style` HTML attribute via
//! `set_property("style", ...)`. This is an alternative to the `style:`
//! shorthand:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     rsx! {
//!         let root = div(style = "cursor: pointer; color: red;") {
//!             "Styled div"
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! # XML Namespaces and SVG
//!
//! The `xmlns = expr` attribute triggers element creation via
//! [`ViewElement::new_namespace`] instead of [`ViewElement::new`]. This is
//! required for SVG elements (and any namespaced XML). The namespace must be
//! supplied on each element that needs it:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() -> V::Element {
//!     let ns = "http://www.w3.org/2000/svg";
//!     rsx! {
//!         let root = svg(xmlns = ns, width = "100", height = "100") {
//!             circle(
//!                 xmlns = ns,
//!                 cx = "50", cy = "50", r = "40",
//!                 stroke = "green",
//!                 stroke_width = "4",
//!                 fill = "yellow"
//!             ){}
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! Note that `stroke_width` becomes `stroke-width` through underscore-to-dash
//! conversion.
//!
//! # Block Expressions
//!
//! `{ rust_expression }` in node position evaluates the expression and
//! appends the result as a child. The expression must evaluate to a type
//! implementing [`ViewChild`]. This is how you nest custom components, render
//! lists, render conditionals, and produce dynamic text.
//!
//! ## Nesting a custom component
//!
//! Any type implementing [`ViewChild`] can be used in a block expression.
//! Components typically derive `ViewChild` (see
//! [Nesting Components](#nesting-components)):
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! #[derive(ViewChild)]
//! struct Counter<V: View> {
//!     #[child]
//!     wrapper: V::Element,
//! }
//!
//! impl<V: View> Default for Counter<V> {
//!     fn default() -> Self {
//!         rsx! {
//!             let wrapper = button() { "Click me." }
//!         }
//!         Self { wrapper }
//!     }
//! }
//!
//! fn view<V: View>() -> V::Element {
//!     rsx! {
//!         let root = div() {
//!             "Application"
//!             br(){}
//!             let counter = {Counter::default()}
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! Here `let counter = {Counter::default()}` both constructs the component
//! and appends it as a child of the `div`. The `counter` variable is
//! available after the macro for further interaction.
//!
//! ## Conditionals via `Option`
//!
//! [`Option<T: ViewChild>`] implements [`ViewChild`]: `Some` renders the
//! node, `None` renders nothing. This enables conditionals via block
//! expressions that return `Option`:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! fn view<V: View>(show_header: bool) -> V::Element {
//!     let maybe_header = if show_header {
//!         Some({
//!             rsx! {
//!                 let h = h1() { "Welcome" }
//!             }
//!             h
//!         })
//!     } else {
//!         None
//!     };
//!
//!     rsx! {
//!         let root = main() {
//!             {maybe_header}
//!             p() { "Content" }
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! ## Lists via `Vec`
//!
//! [`Vec<T: ViewChild>`] implements [`ViewChild`]: each element is appended.
//! Collect an iterator into a `Vec` and use it as a block expression:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! #[derive(ViewChild)]
//! struct Item<V: View> {
//!     #[child]
//!     wrapper: V::Element,
//! }
//!
//! impl<V: View> Item<V> {
//!     fn new(label: &str) -> Self {
//!         rsx! {
//!             let wrapper = li() { {label.to_string().into_text::<V>()} }
//!         }
//!         Self { wrapper }
//!     }
//! }
//!
//! fn view<V: View>() -> V::Element {
//!     let items = ["a", "b", "c"]
//!         .iter()
//!         .map(|s| Item::<V>::new(s))
//!         .collect::<Vec<_>>();
//!
//!     rsx! {
//!         let root = ul() {
//!             {items}
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! For dynamic lists (add/remove at runtime), use imperative
//! [`ViewParent::append_child`] / [`ViewParent::remove_child`] calls on a
//! captured parent element instead of `rsx!` syntax.
//!
//! # Proxy: Reactive Updates
//!
//! [`Proxy<T>`](crate::proxy::Proxy) is mogwai's lightweight reactivity
//! primitive. It holds a piece of state and registers update callbacks. When
//! you change the state, all registered views update automatically. The
//! `rsx!` macro has special syntax for using `Proxy` in both attribute and
//! node positions.
//!
//! Construct a `Proxy` outside the macro, then reference it inside:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! struct Counter<V: View> {
//!     root: V::Element,
//!     on_click: V::EventListener,
//!     clicks: Proxy<u32>,
//! }
//!
//! impl<V: View> Default for Counter<V> {
//!     fn default() -> Self {
//!         let mut clicks = Proxy::default();
//!         rsx! {
//!             let root = button(
//!                 style:cursor = "pointer",
//!                 on:click = on_click
//!             ) {
//!                 {clicks(n => match *n {
//!                     1 => "Click again.".to_string(),
//!                     n => format!("Clicked {n} times."),
//!                 })}
//!             }
//!         }
//!         Self { root, on_click, clicks }
//!     }
//! }
//! ```
//!
//! The proxy syntax is: `proxy_ident(pattern => expression)`. The `pattern`
//! binds a `&T` (a reference to the current model), and the `expression`
//! produces the value to render. The expression is evaluated once immediately
//! (for the initial render) and again whenever the proxy is updated.
//!
//! ## Proxy in Attribute Position
//!
//! In attribute position, `proxy(pat => expr)` sets the attribute's initial
//! value and registers an update for future changes. Only plain attributes
//! and `style:` are supported:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! #[derive(Debug, PartialEq)]
//! struct Status {
//!     color: String,
//!     is_visible: bool,
//! }
//!
//! struct Widget<V: View> {
//!     root: V::Element,
//!     state: Proxy<Status>,
//! }
//!
//! impl<V: View> Widget<V> {
//!     fn new() -> Self {
//!         let mut state = Proxy::new(Status {
//!             color: "black".to_string(),
//!             is_visible: true,
//!         });
//!         rsx! {
//!             let root = div(
//!                 class = state(s => if s.is_visible { "visible" } else { "hidden" }.to_string()),
//!                 style:color = state(s => s.color.clone())
//!             ) {
//!                 "content"
//!             }
//!         }
//!         Self { root, state }
//!     }
//! }
//! ```
//!
//! Supported attribute keys for proxies:
//!
//! - `name = proxy(...)` - reactive property via `set_property`
//! - `style:name = proxy(...)` - reactive style via `set_style`
//!
//! Other multi-segment keys are **not** supported for proxies and produce a
//! compile error.
//!
//! ## Proxy in Node Position
//!
//! In node position, `{ proxy_ident(pat => expr) }` creates a
//! [`ProxyChild`](crate::proxy::ProxyChild) that is diffed and replaced
//! when the proxy updates. The expression must evaluate to a type
//! implementing [`ViewChild`]. This works for text, elements, components,
//! `Option`, `Vec`, etc.
//!
//! ```rust
//! use mogwai::ssr::prelude::*;
//!
//! #[derive(Debug, PartialEq)]
//! struct Status {
//!     color: String,
//!     message: String,
//! }
//!
//! struct Widget<V: View> {
//!     root: V::Element,
//!     state: Proxy<Status>,
//! }
//!
//! fn new_widget<V: View>() -> Widget<V> {
//!     let mut state = Proxy::new(Status {
//!         color: "black".to_string(),
//!         message: "Hello".to_string(),
//!     });
//!
//!     rsx! {
//!         let root = div() {
//!             p(
//!                 id = "message_wrapper",
//!                 style:color = state(s => &s.color)
//!             ) {
//!                 {state(s => &s.message)}
//!             }
//!         }
//!     }
//!
//!     Widget { root, state }
//! }
//!
//! let mut w = new_widget::<mogwai::ssr::Ssr>();
//! assert_eq!(
//!     r#"<div><p id="message_wrapper" style="color: black;">Hello</p></div>"#,
//!     w.root.html_string()
//! );
//!
//! w.state.set(Status {
//!     color: "red".to_string(),
//!     message: "Goodbye".to_string(),
//! });
//! assert_eq!(
//!     r#"<div><p id="message_wrapper" style="color: red;">Goodbye</p></div>"#,
//!     w.root.html_string()
//! );
//! ```
//!
//! The proxy closure body can be a full block expression (not just a single
//! expression):
//!
//! ```rust
//! use mogwai::ssr::prelude::*;
//!
//! struct Widget<V: View> {
//!     root: V::Element,
//!     state: Proxy<String>,
//! }
//!
//! fn new_widget<V: View>() -> Widget<V> {
//!     let mut state = Proxy::new("Hello".to_string());
//!     rsx! {
//!         let root = div() {
//!             {state(s => {
//!                 println!("updating state to: {s}");
//!                 s.clone()
//!             })}
//!         }
//!     }
//!     Widget { root, state }
//! }
//! ```
//!
//! ### Compile error: proxy on the outermost block
//!
//! Using `{ proxy(pat => expr) }` as the outermost (root) node is a compile
//! error, because a `ProxyChild` requires a parent element to attach to:
//!
//! ```compile_fail
//! use mogwai::prelude::*;
//!
//! fn view<V: View>() {
//!     let proxy = Proxy::<u32>::default();
//!     rsx! {
//!         {proxy(_ => "Erroring text node.".into_text::<V>())}
//!     }
//! }
//! ```
//!
//! ## Proxy Semantics
//!
//! - **[`Proxy::set`]** only fires updates if `T: PartialEq` **and** the new
//!   value differs from the current value. Setting the same value is a no-op.
//! - **[`Proxy::modify`]** always fires updates, regardless of whether the
//!   value changed. Use `modify` for in-place mutations.
//! - **[`Proxy`] is not `Clone`.** This is a deliberate design choice to make
//!   tracking data updates easy. The proxy must be moved into the struct that
//!   owns it.
//! - The proxy must be initialized (via `Proxy::new` or `Proxy::default`)
//!   **before** the `rsx!` call, because the initial value is read during
//!   macro expansion.
//!
//! # Nesting Components
//!
//! Any Rust type implementing [`ViewChild`] can be nested as a child in
//! `rsx!`. Components typically derive `ViewChild` using
//! `#[derive(ViewChild)]` on a struct with a `#[child]`-annotated field:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! #[derive(ViewChild)]
//! struct MyComponent<V: View> {
//!     #[child]
//!     wrapper: V::Element,
//! }
//!
//! fn create_view<V: View>(component: &MyComponent<V>) -> V::Element {
//!     rsx! {
//!         let root = div() {
//!             h1() { "Hello, world!" }
//!             {component}
//!         }
//!     }
//!     root
//! }
//! ```
//!
//! ## `ViewProperties` derive
//!
//! To allow property/style manipulation directly on a component (delegating
//! to an inner element), derive `ViewProperties` with a `#[properties]`
//! field:
//!
//! ```rust
//! use mogwai::prelude::*;
//!
//! #[derive(ViewChild, ViewProperties)]
//! struct MyComponent<V: View> {
//!     #[child]
//!     #[properties]
//!     wrapper: V::Element,
//! }
//!
//! fn configure<V: View>(component: &MyComponent<V>) {
//!     component.set_property("class", "active");
//!     component.set_style("color", "red");
//!     assert!(component.has_property("class"));
//! }
//! ```
//!
//! `#[child]` and `#[properties]` can be on the **same** field (the common
//! case) or on **different** fields. When on different fields,
//! `set_property` writes to the `#[properties]` field while `append_child`
//! uses the `#[child]` field.
//!
//! # Lists and Conditionals
//!
//! `rsx!` does not have dedicated `for` loops or `if`/`else` syntax. Instead,
//! use Rust expressions in block position that return types implementing
//! [`ViewChild`]:
//!
//! - **Conditionals**: return `Option<impl ViewChild>` (`Some` renders,
//!   `None` renders nothing). See [Block Expressions](#conditionals-via-option).
//! - **Lists**: collect into `Vec<impl ViewChild>` and use as a block. See
//!   [Block Expressions](#lists-via-vec).
//! - **Dynamic lists**: for add/remove at runtime, capture a parent element
//!   with `let` and call [`ViewParent::append_child`] /
//!   [`ViewParent::remove_child`] imperatively.
//!
//! # Cross-Platform Views
//!
//! `rsx!` expands to calls on `V`'s associated types, where `V: View`. Two
//! implementations are provided:
//!
//! - [`Web`](crate::web::Web): backed by `web_sys` types. Use in browser/WASM
//!   targets.
//! - [`Ssr`](crate::ssr::Ssr): renders to `String` for server-side
//!   rendering. `Ssr::Event = ()`.
//!
//! Write your views generic over `V: View`, then specialize at the call site:
//!
//! ```rust,no_run
//! use mogwai::prelude::*;
//! use mogwai::web::Web;
//! use mogwai::ssr::Ssr;
//!
//! struct Widget<V: View> {
//!     root: V::Element,
//! }
//!
//! impl<V: View> Widget<V> {
//!     fn new() -> Self {
//!         rsx! {
//!             let root = div(class = "my-div") {
//!                 a(href = "http://zyghost.com") { "Schellsan's website" }
//!             }
//!         }
//!         Self { root }
//!     }
//! }
//!
//! let web_element = Widget::<Web>::new();
//! let ssr_element = Widget::<Ssr>::new();
//! println!("{}", ssr_element.root.html_string());
//! ```
//!
//! ## Web specialization
//!
//! Use
//! [`ViewElement::when_element`](crate::view::ViewElement::when_element) to
//! perform web-only operations on generic elements, and
//! [`WebElement::dyn_el`](crate::web::WebElement::dyn_el) to cast to specific
//! `web_sys` types:
//!
//! ```rust,no_run
//! use mogwai::web::prelude::*;
//!
//! struct MyView<V: View> {
//!     root: V::Element,
//!     input: V::Element,
//! }
//!
//! impl<V: View> MyView<V> {
//!     fn new() -> Self {
//!         rsx! {
//!             let root = div(class = "my-view") {
//!                 h1() { "Use the input:" }
//!                 let input = input(type_ = "text") {}
//!             }
//!         }
//!         Self { root, input }
//!     }
//!
//!     fn specialize_for_web(&self) {
//!         self.input.dyn_el(|input: &web_sys::HtmlInputElement| {
//!             let value = input.value();
//!             // do special stuff with the input value here...
//!         });
//!     }
//! }
//! ```
//!
//! # Gotchas and Edge Cases
//!
//! ## `V: View` is required in scope
//!
//! The macro expands to `V::Element::new(...)`, `V::Text::new(...)`, etc. The
//! enclosing function or `impl` block must have a type parameter `V: View`.
//! If `V` is missing, you get a normal Rust "cannot find type `V`" error;
//! there is no special diagnostic from the macro.
//!
//! ## Event listener binding is reversed from intuition
//!
//! `on:click = on_click` **creates** a listener and **binds** it to
//! `on_click`. It does **not** attach a pre-existing handler called
//! `on_click`. The expansion is `let on_click = element.listen("click");`.
//! This is the most common source of confusion for new users.
//!
//! ## `window:` and `document:` listeners are not attached to the element
//!
//! They create global listeners (`V::EventListener::on_window(...)` /
//! `on_document(...)`) independent of the element they are written on. The
//! element is irrelevant; the attribute position is just a syntactic
//! convenience.
//!
//! ## Proxy in attribute position runs immediately
//!
//! The initial value is applied synchronously during macro expansion. The
//! proxy must be initialized before the `rsx!` call. After expansion, an
//! `on_update` callback is registered for future changes.
//!
//! ## `Proxy::set` only fires on change
//!
//! [`Proxy::set`] only triggers updates if `T: PartialEq` and the new value
//! differs. Use [`Proxy::modify`] for guaranteed updates (e.g., incrementing
//! a counter where the mutation is in-place).
//!
//! ## `Proxy` is not `Clone`
//!
//! This is deliberate. A `Proxy` is meant to affect changes within a single
//! component's view, not to share state across components. The proxy must be
//! moved into the owning struct. If you need to coordinate state between
//! components, pass messages or events between them rather than sharing a
//! `Proxy`.
//!
//! ## No fragments
//!
//! The macro parses a single root [`ViewToken`]. You cannot have multiple
//! top-level roots. Use a wrapping element or a block expression returning a
//! `Vec` to achieve fragments.
//!
//! ## Bindings flatten into the enclosing scope
//!
//! The braces in `rsx!` are syntactic delimiters for the parser and tooling,
//! not Rust block scopes. All `let` bindings from the entire tree flatten
//! into the enclosing scope, so names must be unique across the macro
//! invocation. See [Scoping](#scoping).
//!
//! ## No keyed lists
//!
//! There is no `key` attribute for list diffing. Lists are rendered
//! positionally. For dynamic lists, manage children imperatively via
//! `append_child` / `remove_child`.
//!
//! ## No two-way binding
//!
//! There is no `bind:value` or similar syntax. Events are futures; updates
//! are explicit via `Proxy::set` / `modify` or imperative
//! `set_text` / `set_property` calls. Read input values imperatively via
//! [`WebEvent::dyn_ev`](crate::web::WebEvent::dyn_ev).
//!
//! ## No compile-time tag validation
//!
//! `rsx! { foobar() {} }` compiles fine. The error comes at runtime from
//! `V::Element::new("foobar")` if the platform rejects the tag.
//!
//! ## Comments inside RSX
//!
//! Rust line comments (`//`) work inside braced bodies between children
//! because the parser uses `syn`, which handles them. There is no RSX-specific
//! comment syntax.
//!
//! [`View`]: crate::view::View
//! [`ViewElement`]: crate::view::ViewElement
//! [`ViewElement::new`]: crate::view::ViewElement::new
//! [`ViewElement::new_namespace`]: crate::view::ViewElement::new_namespace
//! [`ViewText::new`]: crate::view::ViewText::new
//! [`ViewTextExt::into_text`]: crate::view::ViewTextExt::into_text
//! [`ViewChild`]: crate::view::ViewChild
//! [`ViewProperties`]: crate::view::ViewProperties
//! [`ViewProperties::set_style`]: crate::view::ViewProperties::set_style
//! [`ViewEventListener`]: crate::view::ViewEventListener
//! [`ViewParent::append_child`]: crate::view::ViewParent::append_child
//! [`ViewParent::remove_child`]: crate::view::ViewParent::remove_child
//! [`Proxy::set`]: crate::proxy::Proxy::set
//! [`Proxy::modify`]: crate::proxy::Proxy::modify
//! [`Proxy`]: crate::proxy::Proxy
//! [`ProxyChild`]: crate::proxy::ProxyChild
//! [`ViewToken`]: #syntax-grammar
//! [`Option<T: ViewChild>`]: https://doc.rust-lang.org/std/option/enum.Option.html
//! [`Vec<T: ViewChild>`]: https://doc.rust-lang.org/std/vec/struct.Vec.html

pub use crate::view::{ViewChild, ViewProperties};
pub use mogwai_macros::rsx;
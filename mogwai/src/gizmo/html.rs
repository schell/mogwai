//! Contains [`Gizmo`] constructors for html5 tags.
//!
//! Each of these constructor functions is shorthand for
//! ```rust,ignore
//! Gizmo::element("...")
//! .downcast::<HtmlElement>().ok().unwrap()
//! ```
//!
//! [`Gizmo`]: ../struct.Gizmo.html
use super::{super::utils, Gizmo};
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement};

/// From https://doc.rust-lang.org/rust-by-example/macros/designators.html
macro_rules! tag_constructor {
  ( $func_name:ident ) => {
    pub fn $func_name() -> Gizmo<HtmlElement> {
      let element =
        utils::document()
        .create_element(stringify!($func_name))
        .expect("element")
        .unchecked_into();
      Gizmo::wrapping(element)
    }
  };

  ( $e:ident, $($es:ident),+ ) => {
    tag_constructor!{$e}
    tag_constructor!{$($es),+}
  }
}

// structural
tag_constructor! {
  a,
  article,
  aside,
  body,
  br,
  details,
  div,
  h1,
  h2,
  h3,
  h4,
  h5,
  h6,
  head,
  header,
  hgroup,
  hr,
  html,
  footer,
  nav,
  p,
  section,
  span,
  summary
}

// metadata
tag_constructor! {
  base,
  basefont,
  link,
  meta,
  style,
  title
}

// form
tag_constructor! {
  button,
  datalist,
  fieldset,
  form,
  keygen,
  label,
  legend,
  meter,
  optgroup,
  option,
  select,
  textarea
}

pub fn input() -> Gizmo<HtmlInputElement> {
    let element: HtmlInputElement = utils::document()
        .create_element("input")
        .expect("can't create element")
        .unchecked_into();
    Gizmo::wrapping(element)
}

// formatting
tag_constructor! {
  abbr,
  acronym,
  address,
  b,
  bdi,
  bdo,
  big,
  blockquote,
  center,
  cite,
  code,
  del,
  dfn,
  em,
  font,
  i,
  ins,
  kbd,
  mark,
  output,
  pre,
  progress,
  q,
  rp,
  rt,
  ruby,
  s,
  samp,
  small,
  strike,
  strong,
  sub,
  sup,
  tt,
  u,
  var,
  wbr
}

// list
tag_constructor! {
  dd,
  dir,
  dl,
  dt,
  li,
  ol,
  menu,
  ul
}
// table
tag_constructor! {
  caption,
  col,
  colgroup,
  table,
  tbody,
  td,
  tfoot,
  thead,
  th,
  tr
}

// scripting
tag_constructor! {
  noscript,
  script
}

// embedded content
tag_constructor! {
  applet,
  area,
  audio,
  canvas,
  embed,
  figcaption,
  figure,
  frame,
  frameset,
  iframe,
  img,
  map,
  noframes,
  object,
  param,
  source,
  time,
  video
}

/// Contains macro'd definitions of GizmoBuilder::new("...") for all html5 tags.
use super::GizmoBuilder;

/// From https://doc.rust-lang.org/rust-by-example/macros/designators.html
macro_rules! tag_constructor {
  ( $func_name:ident ) => {
    pub fn $func_name() -> GizmoBuilder {
      GizmoBuilder::new(stringify!($func_name))
    }
  };

  ( $e:ident, $($es:ident),+ ) => {
    tag_constructor!{$e}
    tag_constructor!{$($es),+}
  }
}

// structural
tag_constructor!{
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
tag_constructor!{
  base,
  basefont,
  link,
  meta,
  style,
  title
}

// form
tag_constructor!{
  button,
  datalist,
  fieldset,
  form,
  input,
  keygen,
  label,
  legend,
  meter,
  optgroup,
  option,
  select,
  textarea
}

// formatting
tag_constructor!{
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
tag_constructor!{
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
tag_constructor!{
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
tag_constructor!{
  noscript,
  script
}

// embedded content
tag_constructor!{
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

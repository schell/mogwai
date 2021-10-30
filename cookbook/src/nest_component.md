# Nesting components
A type implementing [Component][traitcomponent] is a node in a user interface graph.
This type will naturally contain other types that represent other nodes in the graph.
maintaining a [Gizmo][structgizmo] in your component. Then spawn a builder from that sub-component field in your
component's [Component::view][traitcomponent_methodview] function to add the sub-component's view to your component's DOM.

```rust

```

{{#include reflinks.md}}

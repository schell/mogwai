# ☎️  Communication between Logic and View

Most `Component`s will be complex and will use more than one channel for communication.
How you structure the component's communication is up to you. Here is an example of a component that uses two-way
communication to maintain a count of the number of clicks, displaying it with a nice message to
the reader:

```rust, no-run
{{#include ../../examples/nested-components/src/lib.rs:45:68}}
```

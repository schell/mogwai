# 🏢 Facades

Components talk within themselves from the view to the logic and vice versa, but there
are often stakeholders outside the component that would like to make queries, inject state,
or otherwise communicate with the internals of the component.

Because of this, a common pattern emerges where a struct type will contain a `Sender<LogicMsg>`
and will provide an async API to its owner.

```rust
{{#include ../../examples/todomvc/src/app/item.rs:7:11}}
```

```rust
{{#include ../../examples/todomvc/src/app/item.rs:70:78}}
```

This is called a "facade" - an age old pattern from the MVC days. Under the hood the facade
object sends messages to the logic loop that may contain a query and a response channel.
It awaits a response from the logic loop and then relays the message back as the result
of its own async function.

```rust
{{#include ../../examples/todomvc/src/app/item.rs:179:181}}
```

Whoever owns the facade has a way to communicate directly with the logic loop, without having
to expose the entire logic message type to the public. It's also possible to share data between
the logic loop and a facade with the use of the various locks and mutexes.

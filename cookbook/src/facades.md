# 🏢 Facades

Components talk within themselves from the view to the logic and vice versa, but there
are often stakeholders outside the component that would like to make queries, inject state,
or otherwise communicate with the internals of the component.

Because of this, a common pattern emerges where a helper struct type will contain a `Sender<LogicMsg>`
(where `LogicMsg` is the component's logic message type) and the helper struct will provide an async
API to its owner.

```rust, ignore
{{#include ../../examples/todomvc/src/app/item.rs:7:11}}
```

Above you can see a helper struct that contains a private field `tx_logic`. Below you'll see how
the `tx_logic` channel is used to send a query with a response `Sender` to the component that it's
helping.

```rust, ignore
{{#include ../../examples/todomvc/src/app/item.rs:70:78}}
```

This is called a "facade" - an age old pattern from the MVC days. Under the hood the facade
object sends messages to the logic loop that may contain a query and a response channel.
It awaits a response from the logic loop and then relays the message back as the result
of its own async function.

```rust, ignore
{{#include ../../examples/todomvc/src/app/item.rs:179:181}}
```

Whoever owns the facade has a way to communicate directly with the logic loop, without having
to expose the entire logic message type to the public. It's also possible to share data between
the logic loop and a facade with the use of various reference counters, locks and mutexes.

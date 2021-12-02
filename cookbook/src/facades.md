# 🏢 Facades

Components talk within themselves from the view to the logic and vice versa, but there
are often stakeholders outside the component that would like to make queries, inject state,
or otherwise communicate with the internals of the component.

Because of this, a common pattern emerges where a helper struct type will contain a `Sender<LogicMsg>`
(where `LogicMsg` is the component's logic message type) and the helper struct will provide an async
API to its owner.

```rust, ignore
{{#include ../../examples/todomvc/src/app/item.rs:todo_struct}}
```

Above you can see a helper struct that contains a private field `tx_logic`. Below you'll see how
the `tx_logic` channel is used to send a query with a response `Sender` to the component that it's
helping.

```rust, ignore
{{#include ../../examples/todomvc/src/app/item.rs:use_tx_logic}}
```

This is called a "facade" - an age old pattern from the MVC days. Under the hood the facade
object sends messages to the logic loop that may contain a query and a response channel.
It awaits a response from the logic loop and then relays the message back as the result
of its own async function.

```rust, ignore
{{#include ../../examples/todomvc/src/app/item.rs:facade_logic_loop}}
```

Whoever owns the facade has a way to communicate directly with the logic loop, without having
to expose the entire logic message type to the public. It's also possible to share data between
the logic loop and a facade with the use of various reference counters, locks and mutexes.

## Not just for logic loops
Facades are not just for logic loops. We can also use them for our views as a
layer that hides the use of channels to communicate with the DOM. In fact, the `struct_view` macro is a nifty tool available in `mogwai >= 0.6` that can be used to generate a view facade for you:

```rust, no_run
# use mogwai::prelude::*;

struct_view! {
    <MyView>
        <div
         on:click = get_click >
             {set_text}
        </div>
    </MyView>
}
```

This generates a struct `MyView<T: Eventable>` with a number of methods for interacting with
the view:

- `pub async fn get_click(&self) -> T::Event`
- `pub fn get_click_stream(&self) -> impl Stream<Item = T::Event>`

- `pub async fn set_text(&self)`
- `pub fn set_text_with_stream(&self, stream: impl Stream<Item = String>)`

- `pub fn new() -> (MyView<T>, ViewBuilder<T>)`
- `pub async fn get_inner(&self) -> T`

The generated `MyView<T>` is `Clone`. It can be used inside a logic loop to send updates to the DOM.

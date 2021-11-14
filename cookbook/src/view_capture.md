# 🕸️  Capturing parts of the View

Views often contain nodes that are required in the logic loop. When a view node is needed in a
logic loop we can capture it using a channel.

The view in question should take a `Sender<Dom>` (or whatever the underlying view type is) and
then use it in a `post:build` operation like so:

```rust, no_run
# use mogwai::prelude::*;

fn view(send_input: broadcast::Sender<Dom>) -> ViewBuilder<Dom> {
    builder! {
        <div>

            <button post:build=move |dom: &mut Dom| { send_input.try_broadcast(dom.clone()).unwrap(); } >

                "Click me"

            </button>

        </div>
    }
}
```

The function given to the `post:build` is run after the view's node is created and before it is attached to any
parent views. This is just one example of how to use the `post:build` RSX attribute, it's quite useful!

We can retrieve the node from the `Receiver` side at the beginning of the logic loop as follows:

```rust, no_run
# use mogwai::prelude::*;

async fn logic(mut recv_input: broadcast::Receiver<Dom>) {
    let input: Dom = recv_input.next().await.unwrap();

    loop {
        // ... do our logic as normal
    }
}
```

Here is a example excerpt taken from [mogwai's todomvc implementation](https://github.com/schell/mogwai/blob/master/examples/todomvc/src/app.rs):

```rust, ignore
{{#include ../../examples/todomvc/src/app.rs:147:164}}
```

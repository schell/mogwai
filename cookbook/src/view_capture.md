# 🕸️  Capturing parts of the View

Views often contain nodes that are required in the logic loop. When a view node is needed in a
logic loop we can capture it using a channel.

## Using the `capture:view` attribute

To capture a view after it is built you can use the [`capture:view`](rsx.md) attribute
with an `impl Sink<T>`, where `T` is your domain view type, and then await the first message on the
receiver:

```rust
# use mogwai::prelude::*;
smol::block_on(async {
    let (tx, mut rx) = broadcast::bounded::<Dom>(1);

    let builder = builder! {
        <div capture:view = tx.sink()></div>
    };

    Component::from(builder)
        .build()
        .unwrap()
        .run()
        .unwrap();

    let _:Dom = rx.next().await.unwrap();
});
```

## Using the `post:build` attribute

The above example is shorthand for using a post-build operation on the view in question.

The view builder should take a `Sender<Dom>` (or whatever the underlying view type is) and
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

# Components
The [Component][traitcomponent] trait allows rust types to be used as a node
in the DOM. These nodes are loosely referred to as components.

## Discussion
A type that implements [Component][traitcomponent] is the state of a user
interface element - for example we might use the following struct to express a
component that keeps track of how many clicks a user has performed on a button.
```rust
struct ButtonCounter {
    clicks: u32
}
```
or `current_contents: String` to store what a
user has entered into an input field. Another common pattern is to use a field like
`items: Gizmo<String>` to be able to communicate with a list of subcopmonents (we'll go
into more detail on that in the [subcomponents](nest_component.md) chapter).

There are two basic concepts you must understand about components:

1. **State**
   As described above, a component probably contains some stateful information which
   determines how that component reacts to input messages (also known as your component's
   [Self::ModelMsg][traitcomponent_atypemodelmsg] type).
   In turn this determines when and if the component sends output messages (also known as
   your component's [Self::ViewMsg][traitcomponent_atypeviewmsg]) to its view.
2. **View**
   A component describes the structure of its DOM, most likely using [RSX](rsx.md).
   It also describes how messages received may change that view and when or if the view
   sends input messages to update the component's state.

To these ends there are two [Component][traitcomponent] methods that you must implement:

1. **[Component::update][traitcomponent_methodupdate]**
   This function is your component's logic. It receives an input message
   [msg: &Self::ModelMsg][traitcomponent_atypemodelmsg] from the view or another
   component and can update the component state using `&mut self`.
   Within this function you can send messages out to the view using the provided
   [tx: &Transmitter\<Self::ViewMsg\>][structtransmitter].
   When the view receives these messages it will patch the DOM according to the
   [Component::view][traitcomponent_methodview] function.

   Also provided is a subscriber [sub: &Subscriber<ModelMsg>][structsubscriber] which
   we'll talk more about in the [subcomponents](nest_component.md) chapter.
   At this point it is good to note that
   [with a `Subscriber` you can send async messages][structsubscriber_methodsend_async]
   to `self` - essentially calling `self.update` when the async block yields. This is great for
   sending off requests, for example, and then triggering a state update with the
   response.

2. **[Component::view][traitcomponent_methodview]**
   This function uses a reference to the current state (in the form of `&self`) to return
   its DOM representation: a [ViewBuilder][structviewbuilder]. With the
   [tx: &Transmitter\<Self::ModelMsg\>][structtransmitter] the returned [ViewBuilder][structviewbuilder]
   can send DOM events as messages to update the component state. With the
   [rx: &Receiver\<Self::ViewMsg\>][structreceiver] the return [ViewBuilder][structviewbuilder]
   can receive messages from the component and patch the DOM accordingly.

## Example

```rust
extern crate mogwai;

use mogwai::prelude::*;

#[derive(Clone)]
enum In {
  Click
}

#[derive(Clone)]
enum Out {
  DrawClicks(i32)
}

struct App {
  num_clicks: i32
}

impl Component for App {
  type ModelMsg = In;
  type ViewMsg = Out;
  type DomNode = HtmlElement;

  fn update(&mut self, msg: &In, tx_view: &Transmitter<Out>, _sub: &Subscriber<In>) {
    match msg {
      In::Click => {
        self.num_clicks += 1;
        tx_view.send(&Out::DrawClicks(self.num_clicks));
      }
    }
  }

  fn view(&self, tx: &Transmitter<In>, rx: &Receiver<Out>) -> ViewBuilder<HtmlElement> {
      builder!{
          <button on:click=tx.contra_map(|_| In::Click)>
          {(
              "clicks = 0",
              rx.branch_map(|msg| {
                  match msg {
                      Out::DrawClicks(n) => {
                          format!("clicks = {}", n)
                      }
                  }
              })
          )}
          </button>
      }
  }
}


fn main() {
    let gizmo: Gizmo<App> = Gizmo::from(App{ num_clicks: 0 });
    let view = View::from(gizmo.view_builder());

    gizmo.send(&In::Click);
    gizmo.send(&In::Click);
    gizmo.send(&In::Click);

    println!("{}", view.html_string());

    // In wasm32 we can add the view to the window.body
    if cfg!(target_arch = "wasm32") {
        view.run().unwrap()
    }
}
```

{{#include reflinks.md}}

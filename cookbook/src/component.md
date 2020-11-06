# Components
The [Component][traitcomponent] trait is how ordinary types can become a UI element.
A type that implements `Component` is the state of a UI element - eg. how many clicks
to display or what an input field contains or maybe a list of subcopmonents (we'll go
more in depth on that later). `Component`s have two methods that you must implement.

1. The [Component::update][traitcomponent_methodupdate] method
   This function is your component's UI logic. It receives an input message and can
   update the implementing type's value since it is passing `&mut self`. Within this
   function you can send messages out to the view using the provided
   [Transmitter][structtransmitter] which is often named `tx`. When the view receives
   these messages it will patch the DOM.

   Also provided is a [Subscriber][structsubscriber] which we'll talk more about later.
   At this point it is good to note that [with a `Subscriber` you can send async messages][structsubscriber_methodsend_async]
   to `self` - essentially calling `self.update` when the async block yields. This is great for
   sending off requests, for example, and then triggering a state update with the
   response.
2. The [Component::view][traitcomponent_methodview] method
is fold function (logic), a state variable and a
[ViewBuilder][structviewbuilder] all wrapped up in one.

## Creating a Component
A mogwai component can be created by implementing the [Component][traitcomponent]
trait for any type. That type is the state. Its [Component::update][traitcomponent_methodupdate] function
is the logic. The [Component::view][traitcomponent_methodview] function returns a builder that becomes the view
(or views). There are a couple steps to set up the model-view-controller scenario:

  1. Use [Gizmo::from][structgizmo_implfromt] to turn your state type into a [Gizmo][structgizmo]
     ```rust,ignore
     let gizmo = Gizmo::from(my_data);
     ```
  2. Create a view using a builder from the gizmo
     ```rust,ignore
     let builder_ref: &ViewBuilder<_> = gizmo.view_builder();
     let view = View::from(builder_ref);
     ```
  3. Run the view and communicate with it using the gizmo
     ```rust,ignore
     view.run().unwrap_throw();
     gizmo.send(&MyMessage);
     ```

Alternatively, if you don't have a need to communicate with your view you can create a view
directly from the gizmo with `let view = View::from(gizmo);`.

```rust
extern crate mogwai;
extern crate web_sys;

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

  fn update(&mut self, msg: &In, tx_view: &Transmitter<Out>, _sub: &Subscriber<In>) {
    match msg {
      In::Click => {
        self.num_clicks += 1;
        tx_view.send(&Out::DrawClicks(self.num_clicks));
      }
    }
  }
}


pub fn main() -> Result<(), JsValue> {
    let gizmo: Gizmo<App> = Gizmo::from(App{ num_clicks: 0 });
    let view = View::from(gizmo);
    if cfg!(target_arch = "wasm32") {
        view.run()
    } else {
        Ok(())
    }
}
```

{{#include reflinks.md}}

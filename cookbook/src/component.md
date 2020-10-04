# Components
A [Component][component] is a fold function (logic), a state variable and a [ViewBuilder][builder]
all wrapped up in one, for your convenience!

## Creating a Component
A mogwai component can be created by implementing the [Component][component]
trait for any type. That type is the state. Its [Component::update][update] function
is the logic. The [Component::view][view] function returns a builder that becomes the view
(or views). There are a couple steps to set up the model-view-controller scenario:

  1. Use [Gizmo::from][gizmo_from] to turn your state type into a [Gizmo][gizmo]
     `let gizmo = Gizmo::from(my_data);`
  2. Create a view using a builder from the gizmo
     `let view = View::from(gizmo.view_builder());`
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

[component]:_
[gizmo_component]:_
[gizmo]:_

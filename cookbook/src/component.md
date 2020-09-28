# Components
A [Component][component] is a fold function (logic), a state variable and a [View][view]
all wrapped up in one, for your convenience!

## Creating a Component
A mogwai component can be created by implementing the [Component][component]
trait for any type. That type is the state. Its [Component::update][update] function
is the logic. The [Component::view][view] function becomes the view. Use
[Gizmo::from][gizmo_from] to turn your type into a [Gizmo][gizmo], which can be
`run` or used however you like.

If your component is the top-level gizmo in your application, or if it simply
is the top-level of its gizmo hierarchy, you can run it with `run()`.

In the following example we assume it is the top-level gizmo in the program.

```rust
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
    if cfg!(target_arch = "wasm32") {
        gizmo.run()
    } else {
        Ok(())
    }
}
```

[component]:_
[gizmo_component]:_
[gizmo]:_

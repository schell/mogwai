# Creating a component
A mogwai component can be created by implementing the [Component][component]
trait for a type. Use `into_component()` to turn your type into a
[GizmoComponent][gizmo_component], which can be `run` or added to a [Gizmo][gizmo] hierarchy
using `with`.

If your component is the top-level gizmo in your application, or if it simply
is the top-level of its gizmo hierarchy, you can run it with `run()`.

In the following example we assume it is the top-level gizmo in the program.

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

  fn view(&self, tx: Transmitter<In>, rx:Receiver<Out>) -> Gizmo<HtmlElement> {
    button()
      .tx_on("click", tx.contra_map(|_| In::Click))
      .rx_text("clicks = 0", rx.branch_map(|msg| {
        match msg {
          Out::DrawClicks(n) => {
            format!("clicks = {}", n)
          }
        }
      }))
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
  App{ num_clicks: 0 }
  .into_component()
  .run()
}
```

[component]:_
[gizmo_component]:_
[gizmo]:_

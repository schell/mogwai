# Nesting components
Naturally your app will likely nest components. The simplest way to nest components is by
maintaining a [Gizmo][structgizmo] in your component. Then spawn a builder from that sub-component field in your
component's [Component::view][traitcomponent_methodview] function to add the sub-component's view to your component's DOM.

```rust
extern crate mogwai;
extern crate web_sys;

use mogwai::prelude::*;

#[derive(Clone)]
enum CounterIn {
    Click,
    Reset
}

#[derive(Clone)]
enum CounterOut {
    DrawClicks(i32)
}

struct Counter {
    num_clicks: i32,
}

impl Component for Counter {
    type ModelMsg = CounterIn;
    type ViewMsg = CounterOut;
    type DomNode = HtmlElement;

    fn view(&self, tx: &Transmitter<CounterIn>, rx: &Receiver<CounterOut>) -> ViewBuilder<HtmlElement> {
        builder!{
            <button on:click=tx.contra_map(|_| CounterIn::Click)>
            {(
                "clicks = 0",
                rx.branch_map(|msg| {
                    match msg {
                        CounterOut::DrawClicks(n) => {
                            format!("clicks = {}", n)
                        }
                    }
                })
            )}
            </button>
        }
    }

    fn update(&mut self, msg: &CounterIn, tx_view: &Transmitter<CounterOut>, _sub: &Subscriber<CounterIn>) {
        match msg {
            CounterIn::Click => {
                self.num_clicks += 1;
                tx_view.send(&CounterOut::DrawClicks(self.num_clicks));
            }
            CounterIn::Reset => {
                self.num_clicks = 0;
                tx_view.send(&CounterOut::DrawClicks(0));
            }
        }
    }
}


#[derive(Clone)]
enum In {
    Click
}


#[derive(Clone)]
enum Out {}


struct App {
    counter: Gizmo<Counter>
}


impl Default for App {
    fn default() -> Self {
        let counter: Gizmo<Counter> = Gizmo::from(Counter { num_clicks: 0 });
        App{ counter }
    }
}


impl Component for App {
    type ModelMsg = In;
    type ViewMsg = Out;
    type DomNode = HtmlElement;

    fn view(&self, tx: &Transmitter<In>, rx: &Receiver<Out>) -> ViewBuilder<HtmlElement> {
        builder!{
            <div>
                {self.counter.view_builder()}
                <button on:click=tx.contra_map(|_| In::Click)>"Click to reset"</button>
            </div>
        }
    }

    fn update(&mut self, msg: &In, tx_view: &Transmitter<Out>, _sub: &Subscriber<In>) {
        match msg {
            In::Click => {
                self.counter.send(&CounterIn::Reset);
            }
        }
    }
}


pub fn main() -> Result<(), JsValue> {
    let view = View::from(Gizmo::from(App::default()));
    if cfg!(target_arch = "wasm32") {
        view.run()
    } else {
        Ok(())
    }
}
```

{{#include reflinks.md}}

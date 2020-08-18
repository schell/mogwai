use log::Level;
use std::panic;
use wasm_bindgen::prelude::*;
use mogwai::prelude::*;

use app::App;


// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


mod item {
    #![allow(unused_braces)]
    use mogwai::prelude::*;

    pub struct TenClickItem {
        pub id: u32,
        pub count: u32
    }

    #[derive(Clone)]
    pub enum In {
        Click,
        ClearCount,
        Delete
    }

    #[derive(Clone)]
    pub enum Out {
        SetCount(u32),
        Delete{ id: u32 }
    }

    impl Component for TenClickItem {
        type ModelMsg = In;
        type ViewMsg = Out;
        type DomNode = HtmlElement;

        fn update(&mut self, msg: &In, tx: &Transmitter<Out>, _sub: &Subscriber<In>) {
            match msg {
                In::Click => {
                    if self.count < 10 {
                        self.count += 1;
                    }
                    tx.send(&Out::SetCount(self.count));
                }
                In::ClearCount => {
                    self.count = 0;
                    tx.send(&Out::SetCount(0));
                }
                In::Delete => {
                    tx.send(&Out::Delete{ id: self.id });
                }
            }
        }

        fn view(&self, tx: &Transmitter<In>, rx: &Receiver<Out>) -> View<HtmlElement> {
            view! {
                <li>
                    <button on:click=tx.contra_map(|_| In::Click)>"Increment"</button>
                    <button on:click=tx.contra_map(|_| In::Delete)>"Remove"</button>
                    <br />
                    <p>{(
                            "Waiting for clicks",
                            rx.branch_filter_map(|msg| match msg {
                                Out::SetCount(count) => Some(format!("{} clicks", count)),
                                _ => None
                            })
                       )}</p>
                </li>
            }
        }
    }
}


mod app {
    use mogwai::prelude::*;
    use super::item;

    pub struct App {
        next_id: u32,
        items: Vec<Gizmo<item::TenClickItem>>,
        item_list_element: Option<HtmlElement>
    }

    #[derive(Clone)]
    pub enum In {
        CreatedList(HtmlElement),
        NewItem,
        ClearAll,
        DeleteItem{ id: u32 }
    }

    #[derive(Clone)]
    pub enum Out {
        SetNumItems(usize)
    }

    impl Component for App {
        type ModelMsg = In;
        type ViewMsg = Out;
        type DomNode = HtmlElement;

        fn update(&mut self, msg: &In, tx: &Transmitter<Out>, sub: &Subscriber<In>) {
            match msg {
                In::CreatedList(ul) => {
                    self.item_list_element = Some(ul.clone());
                }
                In::NewItem => {
                    let new_item = item::TenClickItem{ id: self.next_id, count: 0 };
                    let gizmo = Gizmo::from(new_item);
                    sub.subscribe_filter_map(&gizmo.recv, |item_msg| match item_msg {
                        item::Out::Delete{id} => Some(In::DeleteItem{ id: *id }),
                        _ => None,
                    });

                    if let Some(ul) = &self.item_list_element {
                        let _ = ul.append_child(gizmo.dom_ref());
                    }
                    self.items.push(gizmo);
                    self.next_id += 1;

                    tx.send(&Out::SetNumItems(self.items.len()));
                }
                In::ClearAll => {
                    for item in self.items.iter() {
                        item.update(&item::In::ClearCount);
                    }
                }
                In::DeleteItem{ id } => {
                    self.items.retain(|item| item.with_state(|s| s.id != *id));

                    tx.send(&Out::SetNumItems(self.items.len()));
                }
            }
        }

        fn view(&self, tx: &Transmitter<In>, rx: &Receiver<Out>) -> View<HtmlElement> {
            view! {
                <main>
                    <header>
                        <h1>
                            {("0 items", rx.branch_filter_map(|msg| match msg {
                                Out::SetNumItems(count) => {
                                    Some(
                                        if *count == 1 {
                                            "1 item".to_string()
                                        } else {
                                            format!("{} items", count)
                                        }
                                    )
                                }
                            }))}
                        </h1>
                        <button on:click=tx.contra_map(|_| In::NewItem)>"+"</button>
                        <button on:click=tx.contra_map(|_| In::ClearAll)>"Clear All"</button>
                    </header>
                    <section>
                        <ul post:build=tx.contra_map(|el:&HtmlElement| In::CreatedList(el.clone()))></ul>
                    </section>
                </main>
            }
        }
    }

    impl Default for App {
        fn default() -> Self {
            App {
                next_id: 0,
                items: vec![],
                item_list_element: None
            }
        }
    }
}


#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).expect("could not init console_log");

    Gizmo::from(App::default()).run()
}

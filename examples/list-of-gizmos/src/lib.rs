#![allow(unused_braces)]
use log::Level;
use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// One item - keeps track of clicks.
struct Item {
    id: usize,
    clicks: u32,
}

/// An item's model messages.
#[derive(Clone)]
enum ItemIn {
    /// The user clicked
    Click,
    /// The user requested this item be removed
    Remove,
}

/// An item's view messages.
#[derive(Clone)]
enum ItemOut {
    /// Change the number of clicks displayed in the view
    Clicked(u32),
    /// Remove the item from the parent view
    Remove(usize)
}

impl Component for Item {
    type ModelMsg = ItemIn;
    type ViewMsg = ItemOut;
    type DomNode = HtmlElement;

    fn update(
        &mut self,
        msg: &Self::ModelMsg,
        tx: &Transmitter<Self::ViewMsg>,
        _sub: &Subscriber<Self::ModelMsg>,
    ) {
        match msg {
            ItemIn::Click => {
                self.clicks += 1;
                tx.send(&ItemOut::Clicked(self.clicks))
            }
            ItemIn::Remove => {
                tx.send(&ItemOut::Remove(self.id));
            }
        }
    }

    fn view(
        &self,
        tx: &Transmitter<Self::ModelMsg>,
        rx: &Receiver<Self::ViewMsg>,
    ) -> ViewBuilder<Self::DomNode> {
        let clicks_to_string = |clicks| match clicks {
            1 => "1 click".to_string(),
            n => format!("{} clicks", n),
        };
        builder! {
            <li>
                <button style:cursor="pointer" on:click=tx.contra_map(|_| ItemIn::Click)>"Increment"</button>
                <button style:cursor="pointer" on:click=tx.contra_map(|_| ItemIn::Remove)>"Remove"</button>
                " "
                <span>
                {(
                    clicks_to_string(self.clicks),
                    rx.branch_filter_map(move |msg| match msg {
                        ItemOut::Clicked(clicks) => Some(clicks_to_string(*clicks)),
                        _ => None
                    })
                )}
                </span>
            </li>
        }
    }
}

struct List {
    next_id: usize,
    items: Vec<Gizmo<Item>>,
}

#[derive(Clone)]
enum ListIn {
    /// Create a new item
    NewItem,
    /// Remove the item at the given index
    RemoveItem(usize),
}

#[derive(Clone)]
enum ListOut {
    /// Patch the list of items
    PatchItem(Patch<View<HtmlElement>>),
}

impl Component for List {
    type ModelMsg = ListIn;
    type ViewMsg = ListOut;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &ListIn, tx: &Transmitter<ListOut>, sub: &Subscriber<ListIn>) {
        match msg {
            ListIn::NewItem => {
                let item: Item = Item { id: self.next_id, clicks: 0 };
                self.next_id += 1;

                let gizmo: Gizmo<Item> = Gizmo::from(item);
                sub.subscribe_filter_map(&gizmo.recv, |child_msg: &ItemOut| match child_msg {
                    ItemOut::Remove(index) => Some(ListIn::RemoveItem(*index)),
                    _ => None
                });

                let view: View<HtmlElement> = View::from(gizmo.view_builder());
                tx.send(&ListOut::PatchItem(Patch::PushBack { value: view }));
                self.items.push(gizmo);
            }
            ListIn::RemoveItem(id) => {
                let mut may_index = None;
                'find_item_by_id: for (item, index) in self.items.iter().zip(0..) {
                    if &item.state_ref().id == id {
                        may_index = Some(index);
                        tx.send(&ListOut::PatchItem(Patch::Remove{ index }));
                        break 'find_item_by_id;
                    }
                }
                if let Some(index) = may_index {
                    self.items.remove(index);
                }
            }
        }
    }

    fn view(&self, tx: &Transmitter<ListIn>, rx: &Receiver<ListOut>) -> ViewBuilder<HtmlElement> {
        builder! {
            <fieldset>
                <legend>"A List of Gizmos"</legend>
                <button style:cursor="pointer" on:click=tx.contra_map(|_| ListIn::NewItem)>
                    "Create a new item"
                </button>
                <fieldset>
                    <legend>"Items"</legend>
                    <ol patch:children=rx.branch_map(|ListOut::PatchItem(patch)| patch.clone())>
                    </ol>
                </fieldset>
            </fieldset>
        }
    }
}

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let gizmo = Gizmo::from(List { items: vec![], next_id: 0 });
    let view = View::from(gizmo.view_builder());
    if let Some(id) = parent_id {
        let parent = utils::document()
            .get_element_by_id(&id)
            .unwrap();
        view.run_in_container(&parent)
    } else {
        view.run()
    }
}

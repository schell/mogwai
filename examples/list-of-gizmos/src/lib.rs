#![allow(unused_braces)]
use log::Level;
use mogwai::{core::channel::broadcast, prelude::*};
use std::panic;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// An item widget.
/// Keeps track of clicks.
#[derive(Clone, Debug)]
struct Item {
    id: usize,
    clicks: Model<u32>,
}

/// An item's update messages.
#[derive(Clone)]
enum ItemMsg {
    /// The user clicked
    Click,
    /// The user requested this item be removed
    Remove,
}

/// One item's logic loop.
async fn item_logic(
    id: usize,
    clicks: Model<u32>,
    mut from_view: broadcast::Receiver<ItemMsg>,
    to_list: broadcast::Sender<ListMsg>,
) {
    loop {
        match from_view.recv().await {
            Ok(ItemMsg::Click) => {
                clicks.visit_mut(|c| *c += 1).await;
            }
            Ok(ItemMsg::Remove) => {
                to_list.broadcast(ListMsg::RemoveItem(id)).await.unwrap();
                break;
            }
            Err(_) => break,
        }
    }
    log::info!("item {} logic loop is done", id);
}

// ANCHOR: item_view
fn item_view(
    clicks: impl Stream<Item = u32> + Sendable,
    to_logic: broadcast::Sender<ItemMsg>,
) -> ViewBuilder<Dom> {
    builder! {
        <li>
            <button
                style:cursor="pointer"
                on:click=to_logic.clone().contra_map(|_| ItemMsg::Click)>
                "Increment"
            </button>
            <button
                style:cursor="pointer"
                on:click=to_logic.contra_map(|_| ItemMsg::Remove)>
                "Remove"
            </button>
            " "
            <span>
            {
                ("", clicks.map(|clicks| match clicks {
                    1 => "1 click".to_string(),
                    n => format!("{} clicks", n),
                }))
            }
            </span>
        </li>
    }
}
// ANCHOR_END: item_view

/// Create a new item component.
fn item(id: usize, clicks: Model<u32>, to_list: broadcast::Sender<ListMsg>) -> Component<Dom> {
    let (tx, rx) = broadcast::bounded(1);
    Component::from(item_view(clicks.stream(), tx)).with_logic(item_logic(id, clicks, rx, to_list))
}

#[derive(Clone)]
enum ListMsg {
    /// Create a new item
    NewItem,
    /// Remove the item with the given id
    RemoveItem(usize),
}

// ANCHOR: list_logic_coms
/// Launch the logic loop of our list of items.
async fn list_logic(
    input: broadcast::Receiver<ListMsg>,
    tx_patch_children: mpsc::Sender<ListPatch<ViewBuilder<Dom>>>,
) {
    // Set up our communication from items to this logic loop by
    // * creating a list patch model
    // * creating a channel to go from item to list logic (aka here)
    // * creating a side-effect stream (for_each) that runs for each item patch
    // * map patches of Item to patches of builders and send that to our view
    //   through tx_patch_children
    let mut items: ListPatchModel<Item> = ListPatchModel::new();
    let (to_list, from_items) = broadcast::bounded::<ListMsg>(1);
    let to_list = to_list.clone();
    let all_item_patches = items
        .stream()
        .map(move |patch| {
            log::info!("mapping patch for item: {:?}", patch);
            let to_list = to_list.clone();
            patch.map(move |Item { id, clicks }: Item| {
                let to_list = to_list.clone();
                let component = item(id, clicks, to_list);
                let builder: ViewBuilder<Dom> = component.into();
                builder
            })
        })
        .for_each(move |patch| {
            let mut tx_patch_children = tx_patch_children.clone();
            async move {
                tx_patch_children.send(patch).await.unwrap();
            }
        });
    mogwai::spawn(all_item_patches);
    // ANCHOR_END: list_logic_coms
    // ANCHOR: list_logic_loop
    // Combine the input from our view with the input from our items
    let mut input = stream::select_all(vec![input, from_items]);
    let mut next_id = 0;
    loop {
        match input.next().await {
            Some(ListMsg::NewItem) => {
                log::info!("creating a new item");
                let item: Item = Item {
                    id: next_id,
                    clicks: Model::new(0),
                };
                next_id += 1;
                // patch our items easily and _item_patch_stream's for_each runs automatically,
                // keeping the list of item views in sync
                items.list_patch_push(item);
            }
            Some(ListMsg::RemoveItem(id)) => {
                log::info!("removing item: {}", id);
                let mut may_index = None;
                'find_item_by_id: for (item, index) in items.read().await.iter().zip(0..) {
                    if item.id == id {
                        may_index = Some(index);
                        break 'find_item_by_id;
                    }
                }

                if let Some(index) = may_index {
                    // patch our items to remove the item at the index
                    let _ = items.list_patch_remove(index);
                }
            }
            _ => {
                log::error!("Leaving list logic loop - this shouldn't happen");
                break;
            }
        }
    }
    // ANCHOR_END: list_logic_loop
}

// ANCHOR: list_view
fn list_view<T>(to_logic: broadcast::Sender<ListMsg>, children: T) -> ViewBuilder<Dom>
where
    T: Stream<Item = ListPatch<ViewBuilder<Dom>>> + Sendable,
{
    builder! {
        <fieldset>
            <legend>"A List of Gizmos"</legend>
                <button style:cursor="pointer" on:click=to_logic.contra_map(|_| ListMsg::NewItem)>
                "Create a new item"
            </button>
            <fieldset>
                <legend>"Items"</legend>
                <ol patch:children=children>
                </ol>
            </fieldset>
        </fieldset>
    }
}
// ANCHOR_END: list_view

/// Create our list component.
fn list() -> Component<Dom> {
    let (logic_tx, logic_rx) = broadcast::bounded(1);
    let (item_patch_tx, item_patch_rx) = mpsc::bounded(1);
    Component::from(list_view(logic_tx, item_patch_rx))
        .with_logic(list_logic(logic_rx, item_patch_tx))
}

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();
    mogwai::spawn(async {
        let component = list();
        let view = component.build(()).await?.into_inner();

        if let Some(id) = parent_id {
            let parent = mogwai::dom::utils::document()
                .unwrap_js::<web_sys::Document>()
                .get_element_by_id(&id)
                .map(Dom::wrap_js)
                .unwrap();
            view.run_in_container(&parent)
        } else {
            view.run()
        }
    });
}

#![allow(unused_braces)]
use log::Level;
use mogwai::{
    builder::ViewBuilder,
    channel::{
        broadcast::{self, broadcast},
        IntoSenderSink, SinkExt, Stream, StreamExt,
    },
    futures,
    macros::builder,
    model::{ListPatchApply, ListPatchModel, Model},
    patch::ListPatch,
    spawn::Sendable,
    view::{Dom, View},
};
use std::{convert::TryInto, panic};
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// An item widget.
/// Keeps track of clicks.
#[derive(Clone)]
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
async fn item_logic_loop(
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

async fn item_view(
    clicks: impl Stream<Item = u32> + Sendable,
    to_logic: broadcast::Sender<ItemMsg>,
) -> ViewBuilder<Dom> {
    let bldr = builder! {
        <li>
            <button
                style:cursor="pointer"
                on:click=to_logic.sink().with(|_| async{Ok(ItemMsg::Click)})>
                "Increment"
            </button>
            <button
                style:cursor="pointer"
                on:click=to_logic.sink().with(|_| async{Ok(ItemMsg::Remove)})>
                "Remove"
            </button>
            " "
            <span>
            {
                ViewBuilder::text(clicks.map(|clicks| match clicks {
                    1 => "1 click".to_string(),
                    n => format!("{} clicks", n),
                }))
            }
            </span>
        </li>
    };
    log::info!("created item builder");
    bldr
}

#[derive(Clone)]
enum ListMsg {
    /// Create a new item
    NewItem,
    /// Remove the item with the given id
    RemoveItem(usize),
}

/// Launch the logic loop of our list of items.
fn launch_list_loops(
    input: broadcast::Receiver<ListMsg>,
) -> impl Stream<Item = ListPatch<ViewBuilder<Dom>>> + Sendable {
    let mut items: ListPatchModel<Item> = ListPatchModel::new();

    let (to_list, from_items) = broadcast::<ListMsg>(1);
    let item_stream = items.stream().then(move |patch| {
        let to_list = to_list.clone();
        patch.map_future(move |Item { id, clicks }: Item| {
            let to_list = to_list.clone();
            let item_clicks = clicks.clone();
            async move {
                let (to_logic, from_view) = broadcast(1);
                mogwai::spawn::spawn(async move {
                    item_logic_loop(id, item_clicks, from_view, to_list).await
                });
                item_view(clicks.stream(), to_logic).await
            }
        })
    });

    let mut input = futures::stream::select_all(vec![input, from_items]);
    mogwai::spawn::spawn(async move {
        let mut next_id = 0;
        loop {
            match input.next().await {
                Some(ListMsg::NewItem) => {
                    let item: Item = Item {
                        id: next_id,
                        clicks: Model::new(0),
                    };
                    next_id += 1;
                    items.list_patch_push(item);
                }
                Some(ListMsg::RemoveItem(id)) => {
                    let mut may_index = None;
                    'find_item_by_id: for (item, index) in items.read().await.iter().zip(0..) {
                        if item.id == id {
                            may_index = Some(index);
                            break 'find_item_by_id;
                        }
                    }

                    if let Some(index) = may_index {
                        let _ = items.list_patch_remove(index);
                    }
                }
                _ => break,
            }
        }
    });

    item_stream
}

fn list_view<T>(to_logic: broadcast::Sender<ListMsg>, children: T) -> ViewBuilder<Dom>
where
    T: Stream<Item = ListPatch<ViewBuilder<Dom>>> + Sendable,
{
    builder! {
        <fieldset>
            <legend>"A List of Gizmos"</legend>
                <button style:cursor="pointer" on:click=to_logic.sink().with(|_| async{Ok(ListMsg::NewItem)})>
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

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    mogwai::spawn::spawn(async {
        let (list_logic_tx, list_logic_rx) = broadcast(1);
        let children = launch_list_loops(list_logic_rx);
        let view: View<Dom> = list_view(list_logic_tx, children).try_into().unwrap();

        if let Some(id) = parent_id {
            let parent = mogwai::utils::document().get_element_by_id(&id).unwrap();
            view.run_in_container(&parent)
        } else {
            view.run()
        }
        .unwrap()
    });

    Ok(())
}

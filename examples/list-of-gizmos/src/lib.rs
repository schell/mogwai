#![allow(unused_braces)]
use log::Level;
use mogwai_dom::core::{
    model::{ListPatchModel, Model},
    patch::ListPatch,
};
use mogwai_dom::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// ANCHOR: item
/// An id to keep track of item nodes
#[derive(Clone, Copy, Debug, PartialEq)]
struct ItemId(usize);

/// Creats an individual item.
///
/// Takes the id of the item (which will be unique) and an `Output` to send
/// "remove item" click events (so the item itself can inform the parent when
/// it should be removed).
fn item(id: ItemId, remove_item_clicked: Output<ItemId>) -> ViewBuilder {
    let increment_item_clicked = Output::<()>::default();
    let num_clicks = Model::new(0u32);
    rsx! {
        li() {
            button(
                style:cursor = "pointer",
                on:click = increment_item_clicked.sink().contra_map(|_: JsDomEvent| ())
            ) {
                "Increment"
            }
            button(
                style:cursor = "pointer",
                // every time the user clicks, we'll send the id as output
                on:click = remove_item_clicked.sink().contra_map(move |_: JsDomEvent| id)
            ) {
                "Remove"
            }
            {" "}
            span() {
                {
                    ("", num_clicks.stream().map(|clicks| match clicks {
                        1 => "1 click".to_string(),
                        n => format!("{} clicks", n),
                    }))
                }
            }
        }
    }
    .with_task(async move {
        while let Some(_) = increment_item_clicked.get().await {
            num_clicks.visit_mut(|n| *n += 1).await;
        }
        log::info!("item {} loop is done", id.0);
    })
}
// ANCHOR_END: item

fn map_item_patch(
    patch: ListPatch<ItemId>,
    remove_item_clicked: Output<ItemId>,
) -> ListPatch<ViewBuilder> {
    patch.map(|id| item(id, remove_item_clicked.clone()))
}

// ANCHOR: list
/// Our list of items.
///
/// Set up our communication from items to this logic loop by
/// * giving each created item a clone of a shared `Output<ItemId>` to send events from
/// * creating a list patch model to update from two separate async tasks (one to create, one to remove)
/// * receive output "removal" messages and patch the list patch model
/// * receive output "create" messages and patch the list patch model
fn list() -> ViewBuilder {
    let remove_item_clicked = Output::<ItemId>::default();
    let remove_item_clicked_patch = remove_item_clicked.clone();

    let new_item_clicked = Output::<()>::default();

    let items: ListPatchModel<ItemId> = ListPatchModel::new();
    let items_remove_loop = items.clone();

    rsx! {
        fieldset() {
            legend(){ "A List of Gizmos" }
            button(
                style:cursor = "pointer",
                on:click = new_item_clicked.sink().contra_map(|_: JsDomEvent| ())
            ) {
                "Create a new item"
            }
            fieldset() {
                legend(){ "Items" }
                ol(
                    patch:children = items
                        .stream()
                        .map(move |patch| map_item_patch(patch, remove_item_clicked_patch.clone()))
                ){}
            }
        }
    }
    .with_task(async move {
        // add new items
        let mut next_id = 0;
        while let Some(_) = new_item_clicked.get().await {
            log::info!("creating item {}", next_id);
            let id = ItemId(next_id);
            next_id += 1;
            let patch = ListPatch::push(id);
            items.patch(patch).await;
        }
        log::info!("list 'add' loop is done - should never happen");
    }).with_task(async move {
        // remove items
        while let Some(remove_id) = remove_item_clicked.get().await {
            let items_read = items_remove_loop.read().await;
            let index = items_read.iter().enumerate().find_map(|(i, id)| if id == &remove_id {
                Some(i)
            } else {
                None
            }).unwrap();
            drop(items_read);
            log::info!("removing item {} at index {}", remove_id.0, index);
            let patch = ListPatch::remove(index);
            items_remove_loop.patch(patch).await;
        }
        log::info!("list 'remove' loop is done - should never happen");
    })
}
// ANCHOR_END: list

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let component = list();
    let view = JsDom::try_from(component).unwrap();

    log::info!("built");
    if let Some(id) = parent_id {
        let parent = mogwai_dom::utils::document()
            .visit_as::<web_sys::Document, JsDom>(|doc| {
                JsDom::from_jscast(&doc.get_element_by_id(&id).unwrap())
            })
            .unwrap();
        view.run_in_container(parent)
    } else {
        view.run()
    }
    .unwrap();

    log::info!("done!");
}

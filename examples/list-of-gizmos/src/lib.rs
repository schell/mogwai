// ANCHOR: cookbook_list_full
use futures::future::FutureExt;
use log::Level;
use mogwai::web::prelude::*;
use std::{collections::HashMap, panic};
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// ANCHOR: cookbook_list_item
#[derive(ViewChild)]
pub struct Item<V: View> {
    #[child]
    wrapper: V::Element,
    on_click_increment: V::EventListener,
    on_click_remove: V::EventListener,
    clicks: Proxy<V, u32>,
    id: usize,
}

impl<V: View> Item<V> {
    /// Creates an item from a unique identifier.
    fn new(id: usize) -> Self {
        let mut clicks = Proxy::default();

        rsx! {
            let wrapper = li() {
                button(
                    style:cursor = "pointer",
                    on:click = on_click_increment
                ) {
                    "Increment"
                }
                button(
                    style:cursor = "pointer",
                    on:click = on_click_remove
                ) {
                    "Remove"
                }
                " "
                span() {
                    {clicks(n => match n {
                        1 => "1 click".into_text::<V>(),
                        n => format!("{} clicks", n).into_text::<V>(),
                    })}
                }
            }
        }

        Self {
            wrapper,
            on_click_increment,
            on_click_remove,
            clicks,
            id,
        }
    }
}

impl<V: View> Item<V> {
    async fn run_until_removal_request(&mut self) -> usize {
        loop {
            futures::select! {
                _ = self.on_click_increment.next().fuse() => {
                    self.clicks.set(*self.clicks + 1);
                }
                _ = self.on_click_remove.next().fuse() => {
                    return self.id;
                }
            }
        }
    }
}
// ANCHOR_END: cookbook_list_item

#[derive(ViewChild)]
pub struct ItemSet<V: View> {
    #[child]
    fieldset: V::Element,
    ol: V::Element,
    items: HashMap<usize, Item<V>>,
    next_item: usize,
}

impl<V: View> Default for ItemSet<V> {
    fn default() -> Self {
        rsx! {
            let fieldset = fieldset() {
                legend(){ "Items" }
                let ol = ol(){ }
            }
        }

        Self {
            fieldset,
            ol,
            items: Default::default(),
            next_item: 0,
        }
    }
}

impl<V: View> ItemSet<V> {
    async fn next_removed_item(&mut self) -> usize {
        let all_items = self
            .items
            .values_mut()
            .map(|item| Box::pin(item.run_until_removal_request()))
            .collect::<Vec<_>>();

        if all_items.is_empty() {
            // select_all will panic if there are no items, so we just stall here,
            // as nothing can happen until items are added
            futures::future::pending::<()>().await;
        }

        let (id, _, _) = futures::future::select_all(all_items).await;
        id
    }

    fn add_item(&mut self) {
        let id = self.next_item;
        self.next_item += 1;
        let item = Item::new(id);
        self.ol.append_child(&item);
        self.items.insert(id, item);
    }

    fn remove_item(&mut self, id: usize) {
        if let Some(item) = self.items.remove(&id) {
            self.ol.remove_child(&item);
        }
    }
}

// ANCHOR: cookbook_list_view
#[derive(ViewChild)]
pub struct List<V: View> {
    #[child]
    wrapper: V::Element,
    on_click_new_item: V::EventListener,
    item_set: ItemSet<V>,
}

impl<V: View> Default for List<V> {
    fn default() -> Self {
        rsx! {
            let wrapper = fieldset() {
                legend(){ "A List of Gizmos" }
                button(
                    style:cursor = "pointer",
                    on:click = on_click_new_item
                ) {
                    "Create a new item"
                }
                let item_set = {ItemSet::default()}
            }
        }
        Self {
            wrapper,
            on_click_new_item,
            item_set,
        }
    }
}
// ANCHOR_END: cookbook_list_view

impl<V: View> List<V> {
    pub async fn run_until_next_event(&mut self) {
        let Self {
            wrapper: _,
            on_click_new_item,
            item_set,
        } = self;
        futures::select! {
            remove_item_id  =  item_set.next_removed_item().fuse() => {
                item_set.remove_item(remove_item_id);
            }
            _new_item_clicked = on_click_new_item.next().fuse() => {
                item_set.add_item();
            }
        }
    }
}

#[wasm_bindgen]
pub fn run(parent_id: Option<String>) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let mut list = List::<Web>::default();

    if let Some(id) = parent_id {
        let parent = mogwai::web::document()
            .get_element_by_id(&id)
            .unwrap_throw();
        parent.append_child(&list);
    } else {
        mogwai::web::body().append_child(&list);
    }

    log::info!("built");
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            list.run_until_next_event().await;
        }
    });
}
// ANCHOR_END: cookbook_list_full

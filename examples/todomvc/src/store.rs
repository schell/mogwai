use mogwai::web::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use web_sys::Storage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub title: String,
    pub completed: bool,
}

const KEY: &str = "todomvc-mogwai";

pub fn write_items(items: impl IntoIterator<Item = Item>) -> Result<(), JsValue> {
    let items = items.into_iter().collect::<Vec<_>>();
    let str_value = serde_json::to_string(&items).expect("Could not serialize items");
    mogwai::web::window()
        .local_storage()?
        .into_iter()
        .for_each(|storage: Storage| {
            storage
                .set_item(KEY, &str_value)
                .expect("could not store serialized items");
        });
    Ok(())
}

pub fn read_items() -> Result<Vec<Item>, String> {
    let storage = mogwai::web::window()
        .local_storage()
        .map_err(|jsv| format!("{:#?}", jsv))?
        .expect("Could not get local storage");

    let may_item_str: Option<String> = storage.get_item(KEY).expect("Error using storage get_item");

    let items = may_item_str
        .map(|json_str: String| {
            let items: Vec<Item> =
                serde_json::from_str(&json_str).expect("Could not deserialize items");
            items
        })
        .unwrap_or_default();

    Ok(items)
}

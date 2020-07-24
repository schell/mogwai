use mogwai::utils;
use serde::{Deserialize, Serialize};
use serde_json;
use wasm_bindgen::JsValue;
use web_sys::Storage;


#[derive(Serialize, Deserialize)]
pub struct Item {
    pub title: String,
    pub completed: bool,
}

const KEY: &str = "todomvc-mogwai";

pub fn write_items(items: Vec<Item>) -> Result<(), JsValue> {
    let str_value = serde_json::to_string(&items).expect("Could not serialize items");
    utils::window()
        .local_storage()?
        .into_iter()
        .for_each(|storage: Storage| {
            storage
                .set_item(KEY, &str_value)
                .expect("could not store serialized items");
        });
    Ok(())
}

pub fn read_items() -> Result<Vec<Item>, JsValue> {
    let storage = utils::window()
        .local_storage()?
        .expect("Could not get local storage");

    let may_item_str: Option<String> = storage.get_item(KEY).expect("Error using storage get_item");

    let items = may_item_str
        .map(|json_str: String| {
            let items: Vec<Item> =
                serde_json::from_str(&json_str).expect("Could not deserialize items");
            items
        })
        .unwrap_or(vec![]);

    Ok(items)
}

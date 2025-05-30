use serde::{Deserialize, Serialize};
use wasm_bindgen::UnwrapThrowExt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredBench {
    pub name: String,
    pub samples: Vec<f64>,
}

impl StoredBench {
    pub fn get_previous(name: impl AsRef<str>) -> Option<Self> {
        let stored_benches = StoredBenches::get(name);
        stored_benches.benches.last().map(|(_, v)| v.clone())
    }

    pub fn write(&self) {
        let mut stored_benches = StoredBenches::get(&self.name);
        let ts = web_sys::js_sys::Date::now();
        stored_benches.benches.push((ts, self.clone()));
        stored_benches.store(&self.name);
    }

    pub fn average(&self) -> f64 {
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StoredBenches {
    pub benches: Vec<(f64, StoredBench)>,
}

impl StoredBenches {
    pub fn get(name: impl AsRef<str>) -> Self {
        let window = web_sys::window().expect_throw("no window");
        let storage = window
            .local_storage()
            .expect_throw("could not get local storage")
            .expect_throw("missing storage");
        let item = storage
            .get_item(name.as_ref())
            .expect_throw("storage problem");
        item.map(|s| serde_json::from_str(&s).expect_throw("could not deserialize stored benches"))
            .unwrap_or_default()
    }

    pub fn store(&self, key: impl AsRef<str>) {
        let window = web_sys::window().expect_throw("no window");
        let storage = window
            .local_storage()
            .expect_throw("could not get local storage")
            .expect_throw("missing storage");
        let value = serde_json::to_string(self).expect_throw("could not serialize benches");
        storage
            .set_item(key.as_ref(), &value)
            .expect_throw("could not set storage");
    }
}

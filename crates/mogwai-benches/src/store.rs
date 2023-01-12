use anyhow::Context;
use mogwai_dom::utils;
use serde::{Deserialize, Serialize};
use web_sys::Storage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredBench {
    pub name: String,
    pub samples: Vec<f64>,
}

impl StoredBench {
    fn key(&self) -> String {
        self.name.clone()
    }

    pub fn try_load(name: impl ToString) -> anyhow::Result<Option<Self>> {
        let storage = utils::window()
            .local_storage()
            .map_err(|jsv| anyhow::anyhow!("could not get local storage: {:#?}", jsv))?
            .context("no storage")?;
        let key = name.to_string();
        if let Some(s) = storage.get_item(&key).ok().context("storage problem")? {
            let stored_bench: StoredBench = serde_json::from_str(&s)?;
            Ok(Some(stored_bench))
        } else {
            Ok(None)
        }
    }

    pub fn try_write(&self) -> anyhow::Result<()> {
        let str_value = serde_json::to_string(&self)?;
        let key = self.key();
        utils::window()
            .local_storage()
            .map_err(|jsv| anyhow::anyhow!("could not get local storage: {:#?}", jsv))?
            .into_iter()
            .for_each(|storage: Storage| {
                storage
                    .set_item(&key, &str_value)
                    .expect("could not store serialized items");
            });
        Ok(())
    }

    pub fn average(&self) -> f64 {
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }
}

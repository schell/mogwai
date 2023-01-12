use mogwai_dom::prelude::*;
use std::{future::Future, marker::PhantomData, panic};
use wasm_bindgen::prelude::*;

mod benches;
mod store;

struct Bench<F, Fut> {
    name: &'static str,
    // seconds
    samples: Vec<f64>,
    routine: F,
    _phantom: PhantomData<Fut>,
}

impl<F, Fut> Bench<F, Fut>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    pub fn new(name: &'static str, routine: F) -> Self {
        Bench {
            name,
            samples: vec![],
            routine,
            _phantom: PhantomData,
        }
    }

    pub async fn run(&mut self, mut warmups: usize, num_samples: usize) {
        while self.samples.len() < num_samples {
            let start_millis = mogwai_dom::core::time::now();
            let fut = (self.routine)();
            fut.await;
            let end_millis = mogwai_dom::core::time::now();
            if warmups > 0 {
                warmups -= 1;
            } else {
                self.samples.push((end_millis - start_millis) / 1000.0);
            }
        }
    }

    pub fn average(&self) -> f64 {
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let to_store = store::StoredBench {
            name: self.name.to_string(),
            samples: self.samples.clone(),
        };
        to_store.try_write()
    }

    pub fn viewbuilder(&self) -> ViewBuilder {
        rsx! {
            fieldset() {
                legend() { {self.name} }
                dl() {
                    dt() {"Average"}
                    dd() { {format!("{}", self.average())} }
                    {
                        store::StoredBench::try_load(self.name).expect("storage problem").map(|prev| {
                            let avg = self.average();
                            let prev_avg = prev.average();
                            let percent_change = 100.0 * (avg - prev_avg) / prev_avg;
                            let change_class = if percent_change.abs() > 3.0 {
                                if percent_change.signum() < 0.0 {
                                    "change-green"
                                } else {
                                    "change-red"
                                }
                            } else {
                                "change"
                            };
                            rsx!{
                                slot() {
                                    dt() { "Change" }
                                    dd(class = change_class) {
                                        {format!("{:0.03}%", percent_change)}
                                    }
                                }

                            }
                        })
                    }
                    {
                        self.samples.iter().enumerate().map(|(i, time)| rsx!{
                            slot(){
                                dt() { {format!("{}", i)} }
                                dd() { {format!("{:?}", time)} }
                            }
                        }).collect::<Vec<_>>()
                    }
                }
            }
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not init console_log");

    log::info!("creating Mdl");
    let mdl = benches::Mdl::default();
    let dom = JsDom::try_from(mdl.clone().viewbuilder()).unwrap();
    dom.run().unwrap();

    wasm_bindgen_futures::spawn_local(async move {
        log::info!("creating and running benchmarks");

        let doc = mogwai_dom::utils::document();
        let mut create_1000 = Bench::new("create_1000", || async {
            benches::create(&mdl, &doc, 1000).await;
        });
        let mut create_10_000 = Bench::new("create_10_000", || async {
            benches::create(&mdl, &doc, 10_000).await;
        });

        create_1000.run(3, 7).await;
        create_10_000.run(1, 2).await;

        let stats = JsDom::try_from(rsx! {
            fieldset(id = "results") {
                legend() { "Results" }
                {create_1000.viewbuilder()}
                {create_10_000.viewbuilder()}
            }
        })
        .expect("could not build stats");
        stats.run().unwrap();
        create_1000.save().expect("could not save result");
        create_10_000.save().expect("could not save result");

        log::info!("done!");
    });

    Ok(())
}

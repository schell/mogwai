use mogwai_dom::prelude::*;
use std::{future::Future, panic, pin::Pin};
use wasm_bindgen::prelude::*;

mod benches;
mod store;

pub struct Bench<'a> {
    name: &'static str,
    // seconds
    samples: Vec<f64>,
    routine: Box<dyn FnMut() -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a>,
}

impl<'a> Default for Bench<'a> {
    fn default() -> Self {
        Self {
            name: "unknown",
            samples: Default::default(),
            routine: Box::new(|| Box::pin(async {})),
        }
    }
}

impl<'a> Bench<'a> {
    pub fn new<F, Fut>(name: &'static str, f: F) -> Self
    where
        F: FnMut() -> Fut + 'a,
        Fut: Future<Output = ()> + 'a,
    {
        Bench::default()
            .with_name(name)
            .with_routine(f)
    }

    pub fn with_routine<F, Fut>(mut self, mut f: F) -> Self
    where
        F: FnMut() -> Fut + 'a,
        Fut: Future<Output = ()> + 'a,
    {
        self.routine = Box::new(move || Box::pin(f()));
        self
    }

    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
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

#[derive(Default)]
pub struct BenchSet<'a> {
    benches: Vec<Bench<'a>>,
}

impl<'a> BenchSet<'a> {
    pub fn with_bench(mut self, bench: Bench<'a>) -> Self {
        self.benches.push(bench);
        self
    }

    pub async fn run(&mut self, warmups: usize, iters: usize) {
        for bench in self.benches.iter_mut() {
            bench.run(warmups, iters).await;
        }
    }

    pub fn viewbuilder(&self) -> ViewBuilder {
        rsx! {
            fieldset(id = "results") {
                legend() { "Results" }
                {
                    self.benches.iter().map(Bench::viewbuilder).collect::<Vec<_>>()
                }
            }
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        for bench in self.benches.iter() {
            bench.save()?;
        }
        Ok(())
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not init console_log");

    log::info!("creating Mdl");
    let mdl = benches::Mdl::default();
    let dom = JsDom::try_from(mdl.clone().viewbuilder()).unwrap();
    dom.run().unwrap();

    wasm_bindgen_futures::spawn_local(async move {
        log::info!("creating and running benchmarks");

        let doc = mogwai_dom::utils::document();
        let mut set = BenchSet::default()
            .with_bench(
                Bench::new("create_1000", || async {
                    benches::create(&mdl, &doc, 1000).await;
                })
            )
            .with_bench(
                Bench::new("create_10_000", || async {
                    benches::create(&mdl, &doc, 10_000).await;
                })
            );
        set.run(3, 7).await;

        let stats = JsDom::try_from(set.viewbuilder())
            .expect("could not build stats");
        stats.run().unwrap();
        set.save().expect("could not save");

        log::info!("done!");
    });
}

use mogwai_dom::prelude::*;
use std::{future::Future, panic, pin::Pin};
use wasm_bindgen::prelude::*;

mod app;
mod benches;
mod store;

pub struct Time(f64);

impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let log = self.0.log10();
        let mut float = self.0;
        let unit = if log <= -9.0 {
            float *= 1.0e9;
            "ns"
        } else if log <= -6.0 {
            float *= 1.0e6;
            "μs"
        } else if log <= -3.0 {
            float *= 1.0e3;
            "ms"
        } else {
            "s"
        };
        f.write_fmt(format_args!("{:0.03}{} ({:?}s)", float, unit, self.0))
    }
}

pub struct Bench<'a> {
    name: &'static str,
    // seconds
    samples: Vec<f64>,
    warmups: usize,
    iters: usize,
    routine: Box<dyn FnMut() -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a>,
}

impl<'a> Default for Bench<'a> {
    fn default() -> Self {
        Self {
            name: "unknown",
            samples: Default::default(),
            warmups: 3,
            iters: 10,
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

    pub fn with_warmups(mut self, warmups: usize) -> Self {
        self.warmups = warmups;
        self
    }

    pub fn with_iters(mut self, iters: usize) -> Self {
        self.iters = iters;
        self
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

    pub async fn run(&mut self) {
        let num_samples = self.iters;
        let mut warmups = self.warmups;
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

    fn save_name(&self) -> String {
        format!("{}-{}", self.name, if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let to_store = store::StoredBench {
            name: self.save_name(),
            samples: self.samples.clone(),
        };
        to_store.try_write()
    }

    pub fn viewbuilder(&self) -> ViewBuilder {
        rsx! {
            fieldset() {
                legend() {
                    {self.name}
                    {" "}
                    {
                        if cfg!(debug_assertions) {
                            "(debug)"
                        } else {
                            "(release)"
                        }
                    }
                }
                dl() {
                    dt() {"Average"}
                    dd() { {format!("{}", Time(self.average()))} }
                    {
                        store::StoredBench::try_load(self.save_name()).expect("storage problem").map(|prev| {
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

    pub async fn run(&mut self) {
        for bench in self.benches.iter_mut() {
            bench.run().await;
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

    wasm_bindgen_futures::spawn_local(async move {
        log::info!("creating Mdl");
        let mdl = app::Mdl::default();
        let dom = JsDom::try_from(mdl.clone().viewbuilder()).unwrap();
        dom.run().unwrap();
        mogwai_dom::core::time::wait_millis(100).await;

        log::info!("creating and running benchmarks");

        let doc = mogwai_dom::utils::document();
        let mut set = BenchSet::default()
            //.with_bench(
            //    Bench::new("my_select_all", || async {
            //        let usizes = mogwai_dom::core::stream::iter(vec![0usize, 1, 2, 3]);
            //        let floats = mogwai_dom::core::stream::iter(vec![0f32, 1.0, 2.0, 3.0]);
            //        let chars = mogwai_dom::core::stream::iter(vec!['a', 'b', 'c', 'd']);
            //        #[derive(Debug, PartialEq)]
            //        enum X {
            //            A(usize),
            //            B(f32),
            //            C(char),
            //        }
            //        let stream = mogwai_dom::core::stream::select_all(vec![
            //            usizes.map(X::A).boxed(),
            //            floats.map(X::B).boxed(),
            //            chars.map(X::C).boxed(),
            //        ]).unwrap();
            //        //
            //        stream.collect::<Vec<_>>().await;
            //    })
            //    .with_warmups(10)
            //    .with_iters(10_000)
            //);
            .with_bench(
                Bench::new("create_1000", || {
                    let mut mdl = mdl.clone();
                    let doc = &doc;
                    async move {
                        benches::create(&mut mdl, doc, 1000).await;
                    }
                })
            )
            .with_bench(
                Bench::new("create_10_000", || {
                    let mut mdl = mdl.clone();
                    let doc = &doc;
                    async move {
                        benches::create(&mut mdl, doc, 10_000).await;
                    }
                })
            );
        set.run().await;

        let stats = JsDom::try_from(set.viewbuilder())
            .expect("could not build stats");
        stats.run().unwrap();
        set.save().expect("could not save");

        log::info!("done!");
    });
}

#[cfg(test)]
mod test {
    #[test]
    fn units_sanity() {
        assert_eq!(3.0, 1000f32.log10());
        assert_eq!(-3.0, 0.001f32.log10());
    }
}

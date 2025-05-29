use mogwai_futura::web::prelude::*;
use mogwai_js_framework_benchmark::{App, AppView};
use std::{future::Future, panic, pin::Pin};
use wasm_bindgen::prelude::*;

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

#[derive(ViewChild)]
pub struct ChangeView<V: View = Builder> {
    #[child]
    wrapper: V::Element<web_sys::Element>,
}

#[derive(Clone, ViewChild, FromBuilder)]
pub struct BenchView<V: View = Builder> {
    #[child]
    wrapper: V::Element<web_sys::Element>,
}

pub struct Bench<'a> {
    name: &'static str,
    // seconds
    samples: Vec<f64>,
    warmups: usize,
    iters: usize,
    routine: Box<dyn FnMut() -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a>,
    maybe_prev: Option<store::StoredBench>,
}

impl Default for Bench<'_> {
    fn default() -> Self {
        Self {
            name: "unknown",
            samples: Default::default(),
            warmups: 3,
            iters: 10,
            routine: Box::new(|| Box::pin(async {})),
            maybe_prev: Default::default(),
        }
    }
}

fn now() -> f64 {
    web_sys::js_sys::Date::now()
}

impl<'a> Bench<'a> {
    pub fn new<F, Fut>(name: &'static str, f: F) -> Self
    where
        F: FnMut() -> Fut + 'a,
        Fut: Future<Output = ()> + 'a,
    {
        Bench::default().with_name(name).with_routine(f)
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
        self.maybe_prev = store::StoredBench::try_load(self.save_name()).expect("storage problem");
        self
    }

    pub async fn run(&mut self) {
        log::info!("running bench '{}'", self.name);
        let num_samples = self.iters;
        let mut warmups = self.warmups;
        while self.samples.len() < num_samples {
            log::info!("  warmup: {warmups}");
            log::info!("  sample: {}", self.samples.len());
            let start_millis = now();
            let fut = (self.routine)();
            fut.await;
            let end_millis = now();
            if warmups > 0 {
                warmups -= 1;
            } else {
                self.samples.push((end_millis - start_millis) / 1000.0);
            }
        }
        log::info!("  done.");
    }

    pub fn average(&self) -> f64 {
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    fn save_name(&self) -> String {
        format!(
            "{}-{}",
            self.name,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        )
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let to_store = store::StoredBench {
            name: self.save_name(),
            samples: self.samples.clone(),
        };
        to_store.try_write()
    }

    pub fn change_view(&self, maybe_prev: Option<store::StoredBench>) -> ChangeView {
        if let Some(prev) = maybe_prev {
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
            rsx! {
                let wrapper = slot() {
                    dt() { "Change" }
                    dd(class = change_class) {
                        {format!("{:0.03}%", percent_change).into_text::<Builder>()}
                    }
                }
            }
            ChangeView { wrapper }
        } else {
            ChangeView {
                wrapper: ElementBuilder::new("slot"),
            }
        }
    }

    pub fn bench_view(&self) -> BenchView {
        rsx! {
            let wrapper = fieldset() {
                legend() {
                    {self.name.into_text::<Builder>()}
                    " "
                    {
                        if cfg!(debug_assertions) {
                            "(debug)"
                        } else {
                            "(release)"
                        }.into_text::<Builder>()
                    }
                }
                dl() {
                    dt() {"Average"}
                    dd() { {format!("{}", Time(self.average())).into_text::<Builder>()} }
                    {self.change_view(self.maybe_prev.clone())}
                }
            }
        }
        BenchView { wrapper }
    }
}

#[derive(FromBuilder, ViewChild)]
pub struct BenchSetView<V: View = Builder> {
    #[child]
    wrapper: V::Element<web_sys::Element>,
    #[from(benches => benches.into_iter().map(From::from).collect())]
    benches: Vec<BenchView<V>>,
}

#[derive(Default)]
pub struct BenchSet<'a> {
    benches: Vec<Bench<'a>>,
}

impl<'a> BenchSet<'a> {
    pub fn view(&self) -> BenchSetView {
        rsx! {
            let wrapper = fieldset(id = "results") {
                legend() { "Results" }
                let benches = {
                    self
                        .benches
                        .iter()
                        .map(Bench::bench_view)
                        .collect::<Vec<_>>()
                }
            }
        }
        BenchSetView { wrapper, benches }
    }
    pub fn with_bench(mut self, bench: Bench<'a>) -> Self {
        self.benches.push(bench);
        self
    }

    pub async fn run(&mut self) {
        for bench in self.benches.iter_mut() {
            bench.run().await;
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
    console_log::init_with_level(log::Level::Info).expect("could not init console_log");

    wasm_bindgen_futures::spawn_local(async move {
        log::info!("starting benches");
        let doc = web_sys::window().unwrap().document().unwrap();
        let mut set = BenchSet::default()
            .with_bench(Bench::new("create_1000", || async {
                let mut app = App::default();
                let view: AppView<Web> = AppView::default().into();
                view.init();
                benches::create(&mut app, &view, &doc, 1000).await;
                view.deinit();
            }))
            .with_bench(Bench::new("create_10_000", || async {
                let mut app = App::default();
                let view: AppView<Web> = AppView::default().into();
                view.init();
                benches::create(&mut app, &view, &doc, 10_000).await;
                view.deinit();
            }));
        set.run().await;
        set.save().expect("could not save");

        let stats: BenchSetView<Web> = set.view().into();
        let body = doc.body().unwrap();
        body.append_child(&stats);

        log::info!("done!");
    });
}

// #[cfg(test)]
// mod test {
//     #[test]
//     fn units_sanity() {
//         assert_eq!(3.0, 1000f32.log10());
//         assert_eq!(-3.0, 0.001f32.log10());
//     }
// }

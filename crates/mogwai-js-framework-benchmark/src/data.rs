//! Helpers for creating random rows.
use std::sync::atomic::AtomicUsize;

use crate::row::RowModel;

static ADJECTIVES: &[&str] = &[
    "pretty",
    "large",
    "big",
    "small",
    "tall",
    "short",
    "long",
    "handsome",
    "plain",
    "quaint",
    "clean",
    "elegant",
    "easy",
    "angry",
    "crazy",
    "helpful",
    "mushy",
    "odd",
    "unsightly",
    "adorable",
    "important",
    "inexpensive",
    "cheap",
    "expensive",
    "fancy",
];

static COLOURS: &[&str] = &[
    "red", "yellow", "blue", "green", "pink", "brown", "purple", "brown", "white", "black",
    "orange",
];

static NOUNS: &[&str] = &[
    "table", "chair", "house", "bbq", "desk", "car", "pony", "cookie", "sandwich", "burger",
    "pizza", "mouse", "keyboard",
];

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// An PCG PRNG that is optimized for GPUs, in that it is fast to evaluate and accepts
/// sequential ids as its initial state without sacrificing on RNG quality.
///
/// https://www.reedbeta.com/blog/hash-functions-for-gpu-rendering/
/// https://jcgt.org/published/0009/03/02/
///
/// Thanks to Firestar99 at
/// <https://github.com/Firestar99/nanite-at-home/blob/c55915d16ad3b5b4b706d8017633f0870dd2603e/space-engine-shader/src/utils/gpurng.rs#L19>
pub struct GpuRng(pub u32);

impl GpuRng {
    pub fn new(state: u32) -> GpuRng {
        Self(state)
    }

    pub fn gen(&mut self) -> u32 {
        let state = self.0;
        self.0 = self.0.wrapping_sub(747796405).wrapping_add(2891336453);
        let word = (state >> ((state >> 28) + 4)) ^ state;
        let word = word.wrapping_mul(277803737);
        (word >> 22) ^ word
    }

    pub fn choose(&mut self, vs: &'static [&'static str]) -> &'static str {
        let g = self.gen() as usize;
        let len = vs.len();
        let index = g % len;
        vs[index]
    }
}

pub fn build_data(count: usize) -> Vec<RowModel> {
    let date = web_sys::js_sys::Date::now() as u32;
    let mut rng = GpuRng::new(date);

    let mut data = vec![];
    data.reserve_exact(count);

    let next_id = ID_COUNTER.fetch_add(count, std::sync::atomic::Ordering::Relaxed);

    for id in next_id..next_id + count {
        let adjective: &'static str = rng.choose(ADJECTIVES);
        let colour = rng.choose(COLOURS);
        let noun = rng.choose(NOUNS);
        let capacity = adjective.len() + colour.len() + noun.len() + 2;
        let mut label = String::with_capacity(capacity);
        label.push_str(adjective);
        label.push(' ');
        label.push_str(colour);
        label.push(' ');
        label.push_str(noun);
        let label = label.into();
        let row = RowModel { id, label };
        data.push(row);
    }
    data
}

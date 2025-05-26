//! The mogwai-dom js-framework-benchmark application.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use mogwai_futura::web::prelude::*;
use rand::prelude::*;
use wasm_bindgen::{JsCast, UnwrapThrowExt};
use web_sys::Element;

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

type Id = usize;
type Count = usize;
type Step = usize;

fn build_data(count: usize) -> Vec<(usize, String)> {
    let mut thread_rng = rand::rng();

    let mut data: Vec<(usize, String)> = Vec::new();
    data.reserve_exact(count);

    let next_id = ID_COUNTER.fetch_add(count, Ordering::Relaxed);

    for id in next_id..next_id + count {
        let adjective = ADJECTIVES.choose(&mut thread_rng).unwrap();
        let colour = COLOURS.choose(&mut thread_rng).unwrap();
        let noun = NOUNS.choose(&mut thread_rng).unwrap();
        let capacity = adjective.len() + colour.len() + noun.len() + 2;
        let mut label = String::with_capacity(capacity);
        label.push_str(adjective);
        label.push(' ');
        label.push_str(colour);
        label.push(' ');
        label.push_str(noun);
        let row = (id, label);
        data.push(row);
    }
    data
}

#[derive(Clone)]
pub enum Msg {
    Create(Count),
    Append(Count),
    Update(Step),
    Clear,
    Swap,
    Select(Id),
    Remove(Id),
}

#[derive(Clone)]
pub struct Row {
    input_selected: Input<bool>,
    model_id: Model<usize>,
    model_label: Model<String>,
    dom: Captured<JsDom>,
}

impl Row {
    fn new((id, label): (usize, String)) -> Self {
        Row {
            input_selected: Input::new(false),
            model_id: Model::new(id),
            model_label: Model::new(label),
            dom: Default::default(),
        }
    }

    fn get_id_label(&self) -> (usize, String) {
        (
            self.model_id.current().unwrap(),
            self.model_label.current().unwrap(),
        )
    }

    fn set_id_label(&self, id: usize, label: String) {
        self.model_id.try_visit_mut(|prev| {
            *prev = id;
        });
        self.model_label.try_visit_mut(|prev| {
            *prev = label;
        });
    }

    fn viewbuilder(mut self) -> ViewBuilder {
        self.dom
            .current()
            .map(ViewBuilder::from)
            .unwrap_or_else(|| {
                rsx! {
                    tr(
                        key = self.model_id.clone().map(|id| id.to_string()),
                        class = self
                            .input_selected
                            .stream()
                            .unwrap()
                            .map(|is_selected| if is_selected {
                                "danger"
                            } else {
                                ""
                            }.to_string()),
                        capture:view = self.dom.sink()
                    ) {
                        td(class="col-md-1"){{ self.model_id.clone().map(|id| id.to_string()) }}
                        td(class="col-md-4"){
                            a(
                                class = "lbl",
                                key=self.model_id.clone().map(|id| id.to_string())
                            ) {{ self.model_label.clone() }}
                        }
                        td(class="col-md-1"){
                            a(class="remove" ) {
                                span(
                                    class="remove glyphicon glyphicon-remove",
                                    key = self.model_id.clone().map(|id| id.to_string()),
                                    aria_hidden="true"
                                ) {}
                            }
                        }
                        td(class="col-md-6"){ }
                    }
                }
            })
    }
}

/// The main application widget.
#[derive(Clone)]
pub struct App {
    selected: Arc<Mutex<Option<Row>>>,
    cache: Arc<Mutex<Vec<Row>>>,
    rows: ListPatchModel<Row>,
}

impl Default for App {
    fn default() -> Self {
        let rows: ListPatchModel<Row> = ListPatchModel::default();
        Self {
            rows,
            cache: Arc::new(Mutex::new(Vec::with_capacity(11_000))),
            selected: Default::default(),
        }
    }
}

impl App {
    /// Select a new row, deselecting the old row if needed.
    async fn select(&self, row: Option<Row>) {
        log::info!(
            "selecting row: {:?}",
            row.as_ref().map(|r| r.model_label.current()).flatten()
        );
        if let Some(prev_selected_row) = self.selected.lock().unwrap_throw().take() {
            prev_selected_row
                .input_selected
                .try_set(false)
                .expect("can't deselect");
        }
        if let Some(newly_selected_row) = row.as_ref() {
            newly_selected_row
                .input_selected
                .try_set(true)
                .expect("can't select");
        }
        *self.selected.lock().unwrap_throw() = row;
    }

    pub fn dequeue(&self, rows: impl IntoIterator<Item = (usize, String)>) -> Vec<Row> {
        let mut cache = self.cache.lock().unwrap_throw();
        rows.into_iter()
            .map(|(id, label)| {
                if let Some(row) = cache.pop() {
                    row.model_id
                        .try_visit_mut(|i| {
                            *i = id;
                        })
                        .unwrap_throw();
                    row.model_label
                        .try_visit_mut(|l| {
                            *l = label;
                        })
                        .unwrap_throw();
                    row
                } else {
                    Row::new((id, label))
                }
            })
            .collect()
    }

    pub async fn clear(&mut self) {
        self.select(None).await;
        let rows = self.rows.drain().await.expect("could not patch");
        self.cache.lock().unwrap_throw().extend(rows);
    }

    pub async fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Create(cnt) => {
                self.clear().await;
                let rows = self.dequeue(build_data(cnt));
                self.rows.append(rows).await.expect("could not append");
            }
            Msg::Append(cnt) => {
                let rows = self.dequeue(build_data(cnt));
                self.rows.append(rows).await.expect("could not append");
            }
            Msg::Update(step_size) => {
                let rows = self.rows.read().await;
                for row in rows.iter().step_by(step_size) {
                    row.model_label
                        .visit_mut(|label| label.push_str(" !!!"))
                        .await;
                }
            }
            Msg::Clear => {
                self.clear().await;
            }
            Msg::Swap => {
                if self.rows.try_visit(Vec::len).unwrap_throw() > 998 {
                    // This single application supports both keyed and non-keyed implementations.
                    // Which one is used is selected at compilation time using cargo features.
                    if cfg!(feature = "keyed") {
                        // Swap their dom elements by patching the list of rows
                        let row_998 = self
                            .rows
                            .try_patch(ListPatch::remove(998))
                            .unwrap_throw()
                            .pop()
                            .unwrap_throw();
                        let row_1 = self
                            .rows
                            .try_patch(ListPatch::replace(1, row_998))
                            .unwrap_throw()
                            .pop()
                            .unwrap_throw();
                        let _ = self
                            .rows
                            .try_patch(ListPatch::insert(998, row_1))
                            .unwrap_throw();
                    } else {
                        // Swap the text of their ids and labels by updating their fields
                        let rows = self.rows.read().await;
                        let (row1_id, row1_label) = rows[1].get_id_label();
                        let (row998_id, row998_label) = rows[998].get_id_label();
                        rows[1].set_id_label(row998_id, row998_label);
                        rows[998].set_id_label(row1_id, row1_label);
                    }
                }
            }
            Msg::Select(id) => {
                let selected = self
                    .rows
                    .read()
                    .await
                    .iter()
                    .find(|row| row.model_id.current().unwrap_throw() == id)
                    .cloned();
                self.select(selected).await;
            }
            Msg::Remove(remove_id) => {
                let rows = self.rows.read().await;
                let index = rows
                    .iter()
                    .enumerate()
                    .find_map(|(i, row)| -> Option<usize> {
                        let id = row.model_id.current()?;
                        if id == remove_id {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                drop(rows);
                let row = self.rows.remove(index).await.expect("could not patch");
                self.cache.lock().unwrap_throw().push(row);
            }
        }
    }

    pub fn viewbuilder(mut self) -> ViewBuilder {
        // Create one main output click event.
        // We'll use this later to figure out which row was clicked.
        let main_click = Output::<JsDomEvent>::default();
        // Create the DOM using rsx!, which is a nice macro for making nested HTML-like
        // views.
        fn btn(id: &'static str, label: &'static str) -> ViewBuilder {
            rsx!(div(class="col-sm-6 smallpad") {
                button(
                    type="button",
                    class="btn btn-primary btn-block",
                    id = id
                ) { {label} }
            })
        }
        let builder = rsx! (
            div(id="main", on:click = main_click.sink()) {
                div(class="container") {
                    div(class="jumbotron") {
                        div(class="row") {
                            div(class="col-md-6") {
                                h1(){"mogwai"}
                            }
                            div(class="col-md-6") {
                                div(class="row") {
                                    // we can embed any ViewBuilder using curly brackets
                                    {btn("run", "Create 1,000 rows")}
                                    {btn("runlots", "Create 10,000 rows")}
                                    {btn("add", "Append 1,000 rows")}
                                    {btn("update", "Update every 10th row") }
                                    {btn("clear", "Clear")}
                                    {btn("swaprows", "Swap Rows")}
                                }
                            }
                        }
                    }
                    table( class="table table-hover table-striped test-data") {
                        // tbody will have its children patched by a "diff stream" of updates
                        // made to the `rows` field.
                        tbody(patch:children = self
                              .rows
                              .stream()
                              .map(move |patch| patch.map(Row::viewbuilder))
                        ) {}
                    }
                }
            }
        );
        builder.with_task(async move {
            // To save creating thousands of event listeners (one on each row) we instead use one
            // click event on the main div, and then figure out which row was clicked using JS APIs.
            // This is the power of mogwai being so close to the metal.
            while let Some(ev) = main_click.get().await {
                let may_msg = {
                    let e = ev.browser_event().expect("not an event");
                    let target = e
                        .target()
                        .expect("no target")
                        .dyn_into::<Element>()
                        .expect("target not an element");

                    fn get_id(el: &Element) -> Option<Id> {
                        let key = el.get_attribute("key")?;
                        key.parse().ok()
                    }

                    if target.matches("#add").expect("can't match") {
                        e.prevent_default();
                        Some(Msg::Append(1000))
                    } else if target.matches("#run").expect("can't match") {
                        e.prevent_default();
                        Some(Msg::Create(1000))
                    } else if target.matches("#update").expect("can't match") {
                        e.prevent_default();
                        Some(Msg::Update(10))
                    } else if target.matches("#hideall").expect("can't match") {
                        e.prevent_default();
                        None
                    } else if target.matches("#showall").expect("can't match") {
                        e.prevent_default();
                        None
                    } else if target.matches("#runlots").expect("can't match") {
                        e.prevent_default();
                        Some(Msg::Create(10_000))
                    } else if target.matches("#clear").expect("can't match") {
                        e.prevent_default();
                        Some(Msg::Clear)
                    } else if target.matches("#swaprows").expect("can't match") {
                        e.prevent_default();
                        Some(Msg::Swap)
                    } else if target.matches(".remove").expect("can't match") {
                        e.prevent_default();
                        get_id(&target).map(Msg::Remove)
                    } else if target.matches(".lbl").expect("can't match") {
                        e.prevent_default();
                        get_id(&target).map(Msg::Select)
                    } else {
                        None
                    }
                };

                if let Some(msg) = may_msg {
                    self.update(msg).await;
                }
            }
        })
    }
}

use std::sync::atomic::{AtomicUsize, Ordering};

use mogwai_dom::{
    core::model::*,
    prelude::*,
};
use rand::prelude::*;
use wasm_bindgen::JsCast;
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
    let mut thread_rng = thread_rng();

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
}

impl Row {
    fn new((id, label): (usize, String)) -> Self {
        Row {
            input_selected: Input::new(false),
            model_id: Model::new(id),
            model_label: Model::new(label),
        }
    }

    fn get_id_label(&self) -> (usize, String) {
        (self.model_id.current().unwrap(), self.model_label.current().unwrap())
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
                    }.to_string())
            ) {
                td(class="col-md-1"){{ self.model_id.clone().map(|id| id.to_string()) }}
                td(class="col-md-4"){
                    a() {{ self.model_label.clone() }}
                }
                td(class="col-md-1"){
                    a() {
                        span(class="glyphicon glyphicon-remove", aria_hidden="true") {}
                    }
                }
                td(class="col-md-6"){ }
            }
        }
    }
}

#[derive(Clone)]
pub struct Mdl {
    selected: Option<Id>,
    rows: ListPatchModel<Row>,
}

impl Default for Mdl {
    fn default() -> Self {
        let rows: ListPatchModel<Row> = ListPatchModel::default();
        Self { rows, selected: None }
    }
}

impl Mdl {
    async fn select(&mut self, row: Option<usize>) {
        let rows = self.rows.read().await;
        if let Some(prev_selected) = self.selected.take() {
            if let Some(row) = rows.get(prev_selected) {
                row.input_selected.set(false).await.expect("can't deselect");
            }
        }
        if let Some(newly_selected) = row {
            if let Some(row) = rows.get(newly_selected) {
                row.input_selected.set(true).await.expect("can't select");
            }
        }
    }

    pub async fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Create(cnt) => {
                self.select(None).await;
                let new_rows = build_data(cnt).into_iter().map(Row::new);
                self.rows.append(new_rows).await.expect("could not append");
            }
            Msg::Append(cnt) => {
                let new_rows = build_data(cnt).into_iter().map(Row::new);
                self.rows.append(new_rows).await.expect("could not append");
            }
            Msg::Update(step_size) => {
                let rows = self.rows.read().await;
                for row in rows.iter().step_by(step_size) {
                    row.model_label.visit_mut(|label| label.push_str(" !!!")).await;
                }
            }
            Msg::Clear => {
                self.select(None).await;
                self.rows.drain().await.expect("could not patch");
            }
            Msg::Swap => {
                let rows = self.rows.read().await;
                if rows.len() > 998 {
                    // clone them both
                    let (row1_id, row1_label) = rows[1].get_id_label();
                    let (row998_id, row998_label) = rows[998].get_id_label();
                    // switch them both
                    rows[1].set_id_label(row998_id, row998_label);
                    rows[998].set_id_label(row1_id, row1_label);
                }
            }
            Msg::Select(id) => {
                self.select(Some(id)).await;
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
                self.rows.remove(index).await.expect("could not patch");
            }
        }
    }

    pub fn viewbuilder(mut self) -> ViewBuilder {
        let main_click = Output::<JsDomEvent>::default();
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
                                    button(id="run")      { "Create 1,000 rows" }
                                    button(id="runlots")  { "Create 10,000 rows" }
                                    button(id="add")      { "Append 1,000 rows" }
                                    button(id="update")   { "Update every 10th row" }
                                    button(id="clear")    { "Clear" }
                                    button(id="swaprows") { "Swap Rows" }
                                }
                            }
                        }
                    }
                    table( class="table table-hover table-striped test-data") {
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

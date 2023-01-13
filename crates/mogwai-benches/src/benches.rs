use std::sync::atomic::{Ordering, AtomicUsize};

use mogwai_dom::{core::model::*, prelude::*};
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

#[derive(Clone)]
struct Row {
    id: usize,
    label: Model<String>,
}

fn build_data(count: usize) -> Vec<Row> {
    let mut thread_rng = thread_rng();

    let mut data = Vec::new();
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
        data.push(Row {
            id,
            label: Model::new(label),
        });
    }

    data
}

#[derive(Clone)]
enum Msg {
    Create(Count),
    Append(Count),
    Update(Step),
    Clear,
    Swap,
    Select(Id),
    Remove(Id),
}

#[derive(Clone)]
pub struct Mdl {
    selected: Model<Option<Id>>,
    rows: ListPatchModel<Row>,
}

impl Default for Mdl {
    fn default() -> Self {
        let selected: Model<Option<Id>> = Model::new(None);
        let rows: ListPatchModel<Row> = ListPatchModel::default();
        Self {
            rows,
            selected,
        }
    }
}

impl Mdl {
    // ------ ------
    //    Update
    // ------ ------
    async fn update(&self, msg: Msg) {
        match msg {
            Msg::Create(cnt) => {
                self.selected.visit_mut(|s| *s = None).await;
                let new_rows = build_data(cnt);
                for row in new_rows.into_iter() {
                    self.rows.push(row).await.expect("could not create rows");
                }
                //self.rows.splice(.., new_rows).await.expect("could not create
                // rows");
            }
            Msg::Append(cnt) => {
                let new_rows = build_data(cnt);
                let num_rows = self.rows.visit(Vec::len).await;
                self.rows
                    .splice(num_rows.., new_rows)
                    .await
                    .expect("could not append rows");
            }
            Msg::Update(step_size) => {
                let rows = self.rows.read().await;
                for row in rows.iter().step_by(step_size) {
                    row.label.visit_mut(|l| l.push_str(" !!!")).await;
                }
            }
            Msg::Clear => {
                self.selected.visit_mut(|s| *s = None).await;
                self.rows.drain().await.expect("could not patch");
            }
            Msg::Swap => {
                let num_rows = self.rows.visit(Vec::len).await;
                if num_rows > 998 {
                    let row998 = self.rows.remove(998).await.expect("can't remove row 998");
                    let row1 = self
                        .rows
                        .replace(1, row998)
                        .await
                        .expect("could not replace row 1 with previous 998");
                    self.rows
                        .insert(998, row1)
                        .await
                        .expect("could not insert row 1 into index 998");
                }
            }
            Msg::Select(id) => {
                self.selected.visit_mut(|s| *s = Some(id)).await;
            }
            Msg::Remove(remove_id) => {
                let rows = self.rows.read().await;
                let index = rows
                    .iter()
                    .enumerate()
                    .find_map(|(i, row)| if row.id == remove_id { Some(i) } else { None })
                    .unwrap();
                drop(rows);
                self.rows.remove(index).await.expect("could not patch");
            }
        }
    }

    pub fn row_viewbuilder(row: &Row, selected: Model<Option<usize>>, identity: Option<JsDom>) -> ViewBuilder {
        let is_selected = selected.stream().map(move |selected| selected == Some(row.id));
        let select_class = ("", is_selected.map(|is_selected| if is_selected{ "danger"} else { "" }.to_string()));
        let mut builder = rsx!(
            tr(key=row.id.to_string(), class = select_class) {
                td(class="col-md-1"){{ row.id.to_string() }}
                td(class="col-md-4"){
                    a() {{ ("", row.label.stream()) }}
                }
                td(class="col-md-1"){
                    a() {
                        span(class="glyphicon glyphicon-remove", aria_hidden="true") {}
                    }
                }
                td(class="col-md-6"){ }
            }
        );
        if let Some(dom) = identity {
            builder.identity = ViewIdentity::Hydrate(AnyView::new(dom));
        }
        builder
    }

    // ------ ------
    //     View
    // ------ ------
    pub fn viewbuilder(self) -> ViewBuilder {
        let main_click = Output::<JsDomEvent>::default();
        let selected = self.selected.clone();
        let row_node = row_viewbuilder()
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
                        tbody(patch:children = self.rows
                            .stream()
                            .map(move |patch|{
                                patch.map(|row| {})
                            })
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

async fn mdl_create(count: usize, mdl: &Mdl, doc: &JsDom) -> f64 {
    mdl.update(Msg::Create(count)).await;
    let found = mogwai_dom::core::time::wait_for(20.0, || {
        doc.clone_as::<web_sys::Document>()?
            .query_selector(&format!(
                "tbody>tr:nth-of-type({count})>td:nth-of-type(2)>a"
            ))
            .ok()
            .flatten()
    })
    .await
    .expect("cannot create");
    found.elapsed_seconds
}

async fn mdl_clear(mdl: &Mdl, doc: &JsDom) -> f64 {
    mdl.update(Msg::Clear).await;
    let found = mogwai_dom::core::time::wait_for(3.0, || {
        let trs = doc
            .clone_as::<web_sys::Document>()?
            .query_selector_all("tbody>tr")
            .ok()?;
        if trs.length() == 0 {
            Some(())
        } else {
            None
        }
    })
    .await
    .expect("cannot clear");
    found.elapsed_seconds
}

pub async fn create(mdl: &Mdl, doc: &JsDom, count: usize) -> f64 {
    mdl_create(count, mdl, doc).await + mdl_clear(mdl, doc).await
}

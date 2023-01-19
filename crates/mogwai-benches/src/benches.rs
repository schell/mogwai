use std::sync::atomic::{AtomicUsize, Ordering};

use mogwai_dom::{
    core::{either::Either, future::FutureExt, model::*},
    prelude::*,
};
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

#[derive(Clone, PartialEq)]
struct Row {
    id: usize,
    label: String,
}

impl Row {
    fn id_string(self) -> String {
        self.id.to_string()
    }

    fn label(self) -> String {
        self.label
    }
}

fn build_data(count: usize) -> Vec<Row> {
    let mut thread_rng = thread_rng();

    let mut data: Vec<Row> = Vec::new();
    data.reserve_exact(count);

    let next_id = ID_COUNTER.fetch_add(count, Ordering::Relaxed);

    for (i, id) in (next_id..next_id + count).enumerate() {
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
        let row = Row { id, label };
        data.push(row);
    }
    data
}

//
//        if i < current_len {
//            updates.push(async {
//                existing_read[i].visit_mut(|r| {*r = row;}).await;
//            });
//        } else {
//            data.push(Model::new(Row {
//                id,
//                label,
//            }));
//        }
//    }
//
//    let existing_read = existing.read().await;
//    let mut updates = Vec::with_capacity(current_len);
//
//
//}

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
    rows: ListPatchModel<Model<Row>>,
}

impl Default for Mdl {
    fn default() -> Self {
        let selected: Model<Option<Id>> = Model::new(None);
        let rows: ListPatchModel<Model<Row>> = ListPatchModel::default();
        Self { rows, selected }
    }
}

impl Mdl {
    async fn set_rows(&self, mut rows: Vec<Row>) {
        let existing_rows = self.rows.read().await;
        let update_rows = rows.splice(0..existing_rows.len(), []).collect::<Vec<_>>();
        let updates = update_rows
            .into_iter()
            .enumerate()
            .flat_map(|(i, row)| {
                let existing_rows = &existing_rows;
                let existing_row_model = existing_rows.get(i)?.clone();
                Some(
                    async move {
                        existing_row_model
                            .try_visit_mut(|row_model| {
                                *row_model = row;
                            })
                            .expect("can't update row");
                    }
                    .boxed(),
                )
            })
            .collect::<Vec<_>>();
        drop(existing_rows);
        mogwai_dom::core::future::join_all(updates).await;
        self.rows
            .append(rows.into_iter().map(Model::new))
            .await
            .expect("could not append");
    }

    async fn update(&self, msg: Msg) {
        match msg {
            Msg::Create(cnt) => {
                self.selected.replace(None).await;
                let new_rows = build_data(cnt);
                self.set_rows(new_rows).await;
            }
            Msg::Append(cnt) => {
                let new_rows = build_data(cnt);
                self.rows
                    .append(new_rows.into_iter().map(Model::new))
                    .await
                    .expect("could not append");
            }
            Msg::Update(step_size) => {
                let rows = self.rows.read().await;
                for row in rows.iter().step_by(step_size) {
                    row.visit_mut(|existing_row| existing_row.label.push_str(" !!!"))
                        .await;
                }
            }
            Msg::Clear => {
                self.selected.visit_mut(|s| *s = None).await;
                self.rows.drain().await.expect("could not patch");
            }
            Msg::Swap => {
                let rows = self.rows.read().await;
                if rows.len() > 998 {
                    // clone them both
                    let row1: Row = rows[1].try_visit(Clone::clone).expect("can't read row 1");
                    let row998: Row = rows[998].try_visit(Clone::clone).expect("can't read row 1");
                    // switch them both
                    rows[1].replace(row998).await;
                    rows[998].replace(row1).await;
                }
            }
            Msg::Select(id) => {
                self.selected.replace(Some(id)).await;
            }
            Msg::Remove(remove_id) => {
                let rows = self.rows.read().await;
                let index = rows
                    .iter()
                    .enumerate()
                    .find_map(|(i, row)| -> Option<usize> {
                        let id = row.try_visit(|r| r.id)?;
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

    fn row_viewbuilder(row: &Model<Row>, selected: &Model<Option<usize>>) -> ViewBuilder {
        let current_selection = selected.current().expect("can't read selection");
        let current_row = row.current().expect("can't read row");

        let select: PinBoxStream<Either<Option<usize>, usize>> =
            selected.stream().map(Either::Left).boxed();
        let id: PinBoxStream<Either<Option<usize>, usize>> =
            row.stream().map(|row| Either::Right(row.id)).boxed();
        let is_selected =
            select
                .or(id)
                .scan((current_selection, current_row.id), |(may_s, id), e| {
                    match e {
                        Either::Left(s) => {
                            *may_s = s;
                        }
                        Either::Right(i) => {
                            *id = i;
                        }
                    }
                    Some(*may_s == Some(*id))
                });

        let select_class = (
            "",
            is_selected.map(|is_selected| if is_selected { "danger" } else { "" }.to_string()),
        );
        rsx! {
            tr(key = row.clone().map(Row::id_string), class = select_class) {
                td(class="col-md-1"){{ row.clone().map(Row::id_string) }}
                td(class="col-md-4"){
                    a() {{ row.clone().map(Row::label) }}
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

    // ------ ------
    //     View
    // ------ ------
    pub fn viewbuilder(self) -> ViewBuilder {
        let main_click = Output::<JsDomEvent>::default();
        let selected = self.selected.clone();
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
                                patch.map(|row| {
                                    Self::row_viewbuilder(&row, &selected)
                                })
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

//! App type.
use mogwai::web::prelude::*;
use wasm_bindgen::{JsCast, UnwrapThrowExt};

use crate::{data::*, row::*};

#[derive(ViewChild)]
struct AppBtn<V: View> {
    #[child]
    wrapper: V::Element,
}

impl<V: View> AppBtn<V> {
    fn new(id: impl AsRef<str>, label: impl AsRef<str>) -> Self {
        rsx! {
             let wrapper = div(class="col-sm-6 smallpad") {
                button(
                    type="button",
                    class="btn btn-primary btn-block",
                    id = id,
                ) {
                    {label.into_text::<V>()}
                }
            }
        }
        Self { wrapper }
    }
}

pub struct AppView<V: View> {
    pub wrapper: V::Element,
    pub on_click_main: V::EventListener,
    pub rows_tbody: V::Element,
    pub rows_cache: Vec<RowView>,
}

impl<V: View> Default for AppView<V> {
    fn default() -> Self {
        rsx! {
            let wrapper = div(id="main", on:click = on_click_main) {
                div(class="container") {
                    div(class="jumbotron") {
                        div(class="row") {
                            div(class="col-md-6") {
                                h1(){"mogwai"}
                            }
                            div(class="col-md-6") {
                                div(class="row") {
                                    // we can embed any ViewBuilder using curly brackets
                                    {AppBtn::new("run", "Create 1,000 rows")}
                                    {AppBtn::new("runlots", "Create 10,000 rows")}
                                    {AppBtn::new("add", "Append 1,000 rows")}
                                    {AppBtn::new("update", "Update every 10th row") }
                                    {AppBtn::new("clear", "Clear")}
                                    {AppBtn::new("swaprows", "Swap Rows")}
                                }
                            }
                        }
                    }
                    table( class="table table-hover table-striped test-data") {
                        // tbody will have its children updated with the rows.
                        let rows_tbody = tbody() {}
                    }
                }
            }
        }

        Self {
            wrapper,
            on_click_main,
            rows_tbody,
            rows_cache: vec![],
        }
    }
}

impl AppView<Web> {
    pub fn init(&self) {
        let body = mogwai::web::body();
        web_sys::Node::append_child(&body, &self.wrapper).unwrap();
    }

    pub fn deinit(&self) {
        let body = mogwai::web::body();
        web_sys::Node::remove_child(&body, &self.wrapper).unwrap();
    }
}

#[derive(Default)]
pub struct App {
    selected: Option<RowView>,
    cache: Vec<RowView>,
    rows: Vec<RowView>,
}

impl App {
    /// Select a new row, deselecting the old row if needed.
    fn select(&mut self, row: Option<RowView>) {
        if let Some(row) = row.as_ref() {
            log::trace!("selecting row: {}", row.id());
            row.set_selected(true);
        } else {
            log::trace!("deselecting row");
        }
        if let Some(prev_selected_row) = std::mem::replace(&mut self.selected, row) {
            prev_selected_row.set_selected(false);
        }
    }

    /// Clear all rows, adding them to the cache.
    pub fn clear(&mut self, view: &AppView<Web>) {
        self.selected = None;
        self.cache.extend(std::mem::take(&mut self.rows));
        self.select(None);

        view.rows_tbody.clone().set_text_content(None);
    }

    /// Append some number of rows to the view.
    fn append(&mut self, view: &AppView<Web>, count: usize) {
        let Self {
            selected: _,
            cache,
            rows,
        } = self;

        let new_rows = {
            let rows = build_data(count);
            rows.into_iter().map(|model| {
                let row = cache.pop().unwrap_or_default();
                row.set_model(&model);
                row
            })
        };
        for row in new_rows {
            view.rows_tbody.append_child(row.node());
            rows.push(row);
        }
    }

    /// Create some number of rows, clearing the current rows.
    pub fn create(&mut self, view: &AppView<Web>, count: usize) {
        self.clear(view);
        self.append(view, count);
    }

    /// Update rows by a given step size, adding " !!!" to the end of each row.
    pub fn update(&mut self, step_size: usize) {
        for row in self.rows.iter().step_by(step_size) {
            row.update_text();
        }
    }

    /// Swaps the values of row index 1 with row index 998.
    fn swap(&mut self) {
        if self.rows.len() > 998 {
            let row1 = &self.rows[1];
            let row1_id = row1.id();
            let row1_label = row1.fast_label();
            let row998 = &self.rows[998];
            let row998_id = row998.id();
            let row998_label = row998.fast_label();

            row1.set_id(row998_id);
            row1.set_label(row998_label);

            row998.set_id(row1_id);
            row998.set_label(row1_label);
        }
    }

    /// Removes one row.
    fn remove(&mut self, id: &str, view: &AppView<Web>) {
        self.rows.retain_mut(|row| {
            if row.id() == id {
                let row = row.clone();
                web_sys::Node::remove_child(&view.rows_tbody, row.node()).unwrap_throw();
                self.cache.push(row);
                false
            } else {
                true
            }
        });
    }

    pub async fn run(mut self, view: AppView<Web>) {
        // To save creating thousands of event listeners (one on each row) we instead use one
        // click event on the main div, and then figure out which row was clicked using JS APIs.
        // This is the power of mogwai being so close to the metal.
        loop {
            let e = view.on_click_main.next().await;
            let target = e
                .target()
                .expect("no target")
                .dyn_into::<web_sys::Element>()
                .expect("target not an element");

            if target.matches("#add").expect("can't match") {
                log::trace!("add");
                e.prevent_default();
                self.append(&view, 1000);
            } else if target.matches("#run").expect("can't match") {
                log::trace!("create 1000");
                e.prevent_default();
                self.create(&view, 1000);
            } else if target.matches("#update").expect("can't match") {
                log::trace!("update 10");
                e.prevent_default();
                self.update(10);
            } else if target.matches("#hideall").expect("can't match")
                || target.matches("#showall").expect("can't match")
            {
                log::trace!("hide/show");
                e.prevent_default();
            } else if target.matches("#runlots").expect("can't match") {
                log::trace!("create 10,000");
                e.prevent_default();
                self.create(&view, 10_000);
            } else if target.matches("#clear").expect("can't match") {
                log::trace!("clear");
                e.prevent_default();
                self.clear(&view);
            } else if target.matches("#swaprows").expect("can't match") {
                log::trace!("swaprows");
                e.prevent_default();
                self.swap();
            } else if target.matches(".remove").expect("can't match") {
                log::trace!("remove");
                e.prevent_default();
                let el: &web_sys::Element = &target;
                let maybe_key = el.get_attribute("key");
                log::trace!("maybe_key: {maybe_key:?}");
                if let Some(key) = maybe_key {
                    self.remove(&key, &view);
                }
            } else if target.matches(".lbl").expect("can't match") {
                log::trace!("select");
                e.prevent_default();
                let el: &web_sys::Element = &target;
                let key = el.get_attribute("key");
                log::trace!("  key: {key:?}");
                let mut found: Option<RowView> = None;
                if let Some(key) = key {
                    for row in self.rows.iter() {
                        if row.id() == key {
                            found = Some(row.clone());
                            break;
                        }
                    }
                }
                self.select(found);
            } else {
                log::trace!("none");
            }
        }
    }
}

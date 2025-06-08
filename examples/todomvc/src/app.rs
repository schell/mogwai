use std::collections::HashMap;

use futures::FutureExt;
use mogwai::web::prelude::*;
use web_sys::wasm_bindgen::{JsCast, UnwrapThrowExt};

use crate::item::TodoItem;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterShow {
    All,
    Completed,
    Active,
}

impl FilterShow {
    fn maybe_from_url(url: String) -> Option<Self> {
        let ndx = url.find('#').unwrap_or(0);
        let (_, hash) = url.split_at(ndx);
        match hash {
            "#/" => Some(FilterShow::All),
            "#/active" => Some(FilterShow::Active),
            "#/completed" => Some(FilterShow::Completed),
            _ => None,
        }
    }

    fn filter_selected(&self, other: FilterShow) -> &'static str {
        if self == &other {
            "selected"
        } else {
            ""
        }
    }
}

#[derive(ViewChild)]
struct TodoList<V: View> {
    #[child]
    wrapper: V::Element,
    items: HashMap<usize, TodoItem<V>>,
    next_id: usize,
}

impl<V: View> Default for TodoList<V> {
    fn default() -> Self {
        rsx! {
            let wrapper = ul(
                class = "todo-list"

            ) {}
        }

        Self {
            wrapper,
            items: Default::default(),
            next_id: 0,
        }
    }
}

impl<V: View> TodoList<V> {
    async fn run_step(&mut self) -> Option<usize> {
        let steps = self
            .items
            .values_mut()
            .map(|item| Box::pin(item.run_step()))
            .collect::<Vec<_>>();

        if steps.is_empty() {
            // select_all will panic if there are no items, so we just stall here,
            // as nothing can happen until items are added
            futures::future::pending::<()>().await;
        }

        let (maybe_destroy_id, _, _) = futures::future::select_all(steps).await;
        maybe_destroy_id
    }

    fn filter_items(&mut self, filter: FilterShow) {
        for item in self.items.values_mut() {
            item.filter(filter);
        }
    }

    fn add_todo(&mut self, name: String) {
        let id = self.next_id;
        self.next_id += 1;
        let item = TodoItem::new(id, name, false);
        self.wrapper.append_child(&item);
        self.items.insert(id, item);
    }

    fn remove_todo(&mut self, id: usize) {
        if let Some(item) = self.items.remove(&id) {
            self.wrapper.remove_child(&item);
        }
    }

    fn toggle_all(&mut self, complete: bool) {
        for item in self.items.values_mut() {
            item.set_completed(complete);
        }
    }

    fn clear_completed(&mut self) {
        let Self { wrapper, items, .. } = self;
        items.retain(|_id, item| {
            let is_completed = item.get_completed();
            if is_completed {
                wrapper.remove_child(item);
            }
            !is_completed
        });
    }
}

#[derive(ViewChild)]
pub struct App<V: View> {
    #[child]
    wrapper: V::Element,
    todo_input: V::Element,
    todo_list: TodoList<V>,

    should_show_todo_list: Proxy<bool>,
    todo_count: Proxy<usize>,
    filter_show: Proxy<FilterShow>,

    on_change_new_todo: V::EventListener,
    on_click_toggle_all: V::EventListener,
    on_window_hashchange: V::EventListener,
    on_click_clear_completed: V::EventListener,

    toggled_complete: bool,
}

impl<V: View> Default for App<V> {
    fn default() -> Self {
        let mut should_show_todo_list = Proxy::<bool>::new(false);
        let mut todo_count = Proxy::<usize>::new(0);
        let mut filter_show = Proxy::<FilterShow>::new(FilterShow::All);
        rsx! {
            let wrapper = section(id="todo_main", class="todoapp") {
                header(class = "header") {
                    h1() { "todos" }
                    let todo_input = input(
                        class = "new-todo",
                        id = "new-todo",
                        placeholder = "What needs to be done?",
                        on:change = on_change_new_todo,
                    ) {}
                }
                section(
                    class = "main",
                    style:display = should_show_todo_list(
                        should => should_show_bool_to_display_string(*should)
                    )
                ) {
                    // This is the "check all as complete" toggle
                    input(
                        id = "toggle-all",
                        type = "checkbox",
                        class = "toggle-all",
                        on:click = on_click_toggle_all
                    ) {}
                    label(for_ = "toggle-all") { "Mark all as complete" }
                    let todo_list = {TodoList::default()}

                }
                footer(
                    class = "footer",
                    style:display = should_show_todo_list(
                        should => should_show_bool_to_display_string(*should)
                    )
                ) {
                    span(class="todo-count") {
                        strong() {
                            {todo_count(count => num_items_to_user_string(*count))}
                        }
                    }
                    ul(
                        class="filters",
                        window:hashchange = on_window_hashchange
                    ) {
                        li() {
                            a(
                                href = "#/",
                                class = filter_show(filt => filt.filter_selected(FilterShow::All))
                            ) { "All" }
                        }
                        li() {
                            a(
                                href = "#/active",
                                class = filter_show(filt => filt.filter_selected(FilterShow::Active))
                            ) { "Active" }
                        }
                        li() {
                            a(
                                href = "#/completed",
                                class = filter_show(filt => filt.filter_selected(FilterShow::Completed))
                            ) { "Completed" }
                        }
                    }
                    button(
                        class = "clear-completed",
                        style:display = should_show_todo_list(
                            should => should_show_bool_to_display_string(*should)
                        ),
                        on:click = on_click_clear_completed
                    ) { "Clear completed" }
                }
            }
        }

        Self {
            wrapper,
            todo_input,
            on_change_new_todo,
            should_show_todo_list,
            on_click_toggle_all,
            todo_list,
            todo_count,
            filter_show,
            on_window_hashchange,
            on_click_clear_completed,
            toggled_complete: false,
        }
    }
}

impl<V: View> App<V> {
    pub async fn run_step(&mut self) {
        enum Step {
            NewTodo(String),
            DestroyTodo(usize),
            ChangeFilter(FilterShow),
            ToggleAll,
            ClearCompleted,
            None,
        }
        let mut step = Step::None;
        futures::select! {
            ev = self.on_change_new_todo.next().fuse() => {
                ev.when_event::<Web, _>(|ev| {
                    let value = crate::utils::event_input_value(ev).unwrap_throw();
                    step = Step::NewTodo(value);
                });
            }
            maybe_destroy_id = self.todo_list.run_step().fuse() => {
                if let Some(id) = maybe_destroy_id {
                    step = Step::DestroyTodo(id);
                }
            }
            hashchange_ev = self.on_window_hashchange.next().fuse() => {
                hashchange_ev.when_event::<Web,_>(|ev| {
                    let hev = ev.dyn_ref::<web_sys::HashChangeEvent>()?;
                    let hash = hev.new_url();
                    let filter = FilterShow::maybe_from_url(hash)?;
                    step = Step::ChangeFilter(filter);
                    Some(())
                });
            }
            _toggle_all_ev = self.on_click_toggle_all.next().fuse() => {
                step = Step::ToggleAll;
            }
            _clear = self.on_click_clear_completed.next().fuse() => {
                step = Step::ClearCompleted;
            }
        };

        match step {
            Step::NewTodo(name) => {
                self.todo_list.add_todo(name);
                self.todo_input.when_element::<Web, _>(|el| {
                    let input = el.dyn_ref::<web_sys::HtmlInputElement>().unwrap_throw();
                    input.set_value("");
                });
            }
            Step::DestroyTodo(id) => {
                self.todo_list.remove_todo(id);
            }
            Step::ChangeFilter(filter) => {
                log::info!("setting filter: {filter:?}");
                self.filter_show.set(filter);
                self.todo_list.filter_items(filter);
            }
            Step::ToggleAll => {
                self.toggled_complete = !self.toggled_complete;
                let complete = self.toggled_complete;
                self.todo_list.toggle_all(complete);
            }
            Step::ClearCompleted => {
                self.todo_list.clear_completed();
            }
            Step::None => {}
        }

        self.determine_show_todos();
        crate::store::write_items(
            self.todo_list
                .items
                .values()
                .map(|item| crate::store::Item {
                    title: item.get_name(),
                    completed: item.get_completed(),
                }),
        )
        .unwrap_throw();
    }

    fn determine_show_todos(&mut self) {
        let count = self
            .todo_list
            .items
            .values()
            .filter(|item| !item.get_completed())
            .count();
        self.todo_count.set(count);
        self.should_show_todo_list
            .set(!self.todo_list.items.is_empty());
    }

    pub fn add_items(&mut self, items: Vec<crate::store::Item>) {
        for item in items.into_iter() {
            self.todo_list.add_todo(item.title);
        }
        self.determine_show_todos();
    }
}

fn should_show_bool_to_display_string(should_show: bool) -> String {
    if should_show { "block" } else { "none" }.to_string()
}

fn num_items_to_user_string(n: usize) -> String {
    match n {
        0 => "0 items left".to_string(),
        1 => "1 item left".to_string(),
        n => format!("{} items left", n),
    }
}

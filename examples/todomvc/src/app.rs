use std::sync::{atomic::AtomicUsize, Arc};

use mogwai_dom::{core::{stream, model::ListPatchModel}, prelude::*};
use wasm_bindgen::JsCast;
use web_sys::HashChangeEvent;

use crate::{
    item::{TodoItem, TodoItemMsg},
    utils,
};

#[derive(Clone, Debug, PartialEq)]
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

    fn should_show_item_with_completion(&self, completion: bool) -> bool {
        match self {
            FilterShow::All => true,
            FilterShow::Completed => completion,
            FilterShow::Active => !completion,
        }
    }
}

/// A wrapper over `ListPatchModel` that provides some convenience.
#[derive(Clone, Default)]
pub struct Items {
    next_id: Arc<AtomicUsize>,
    output_to_list: Output<TodoItemMsg>,
    inner: ListPatchModel<TodoItem>,
}

impl Items {
    /// Sends the number of items after every patch update.
    fn stream_number_of_items(&self) -> impl Stream<Item = usize> {
        let items = self.inner.clone();
        self.inner.stream().then(move |_| {
            let items = items.clone();
            async move {
                let read = items.read().await;
                read.iter()
                    .filter(|item| item.complete.current().map(|done| !done).unwrap_or(false))
                    .count()
            }
        })
    }

    /// Sends a patch of `ViewBuilder` after every patch update.
    fn stream_of_viewbuilders(&self) -> impl Stream<Item = ListPatch<ViewBuilder>> {
        self.inner
            .stream()
            .map(|patch| patch.map(TodoItem::viewbuilder))
    }

    /// Sends whether the todo list should be shown after every patch update.
    fn stream_should_show_todo_list(&self) -> impl Stream<Item = bool> {
        stream::iter(std::iter::once(false)).chain(self.inner.stream().scan(vec![], |vs, patch| {
            vs.list_patch_apply(patch.map(|_| ()));
            Some(vs.len() > 0)
        }))
    }

    /// Sends whether to show the "complete" button after every patch update.
    fn stream_should_show_complete_button(&self) -> impl Stream<Item = bool> {
        let items = self.inner.clone();
        self.inner.stream().then(move |_| {
            let items = items.clone();
            async move {
                let read = items.read().await;
                read.iter()
                    .any(|item| item.complete.current().expect("can't read item complete"))
            }
        })
    }

    async fn push_new_todo(&self, name: String, complete: bool) {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let item = TodoItem::new(id, name, complete, self.output_to_list.clone());
        let _ = self.inner.patch(ListPatch::push(item)).await;
    }

    /// Task that makes new todos.
    async fn task_make_new_todos(self, output_todo_input_change: Output<JsDomEvent>) {
        // make new todos
        while let Some(ev) = output_todo_input_change.get().await {
            let todo_name =
                utils::event_input_value(ev.clone()).expect("new todo event input value");
            if !todo_name.is_empty() {
                self.push_new_todo(todo_name, false).await;
                let input = utils::event_input(ev).unwrap();
                input.set_value("");
            }
        }
    }

    /// Task toggles all the todo items on or off.
    async fn task_toggle_all(self, output_toggle_all_clicked: Output<()>) {
        while let Some(()) = output_toggle_all_clicked.get().await {
            let items_read = self.inner.read().await;
            // if any items are _not_ done, set the whole list to done
            let done = items_read
                .iter()
                .any(|todo_item: &TodoItem| todo_item.complete.current() == Some(false));
            for item in items_read.iter() {
                item.set_complete(done).await;
            }
            drop(items_read);
            self.inner.refresh().await;
        }
    }

    /// Returns the index of the first completed todo in the list
    async fn find_next_completed(&self) -> Option<usize> {
        let items = self.inner.read().await;
        for (i, item) in items.iter().enumerate() {
            let completed: bool = *item.complete.read().await;
            if completed {
                return Some(i);
            }
        }
        None
    }

    /// Task that clears completed todo items.
    async fn task_clear_completed(self, output_clear_completed_clicked: Output<()>) {
        while let Some(()) = output_clear_completed_clicked.get().await {
            while let Some(index) = self.find_next_completed().await {
                let _ = self.inner.patch(ListPatch::remove(index)).await;
            }
        }
    }

    /// Task that filters items.
    async fn task_filter_items(
        self,
        input_filter_show: FanInput<FilterShow>,
        output_window_onhashchange: Output<JsDomEvent>,
    ) {
        while let Some(ev) = output_window_onhashchange.get().await {
            let url = {
                let ev: web_sys::Event = ev.browser_event().unwrap();
                let ev: HashChangeEvent = ev.dyn_into::<HashChangeEvent>().expect("not hash event");
                ev.new_url()
            };
            if let Some(filter) = FilterShow::maybe_from_url(url) {
                for item in self.inner.read().await.iter() {
                    let completion: bool = *item.complete.read().await;
                    let should_show = filter.should_show_item_with_completion(completion);
                    item.set_visible(should_show).await;
                }
                input_filter_show
                    .set(filter)
                    .await
                    .expect("could not set filter");
            }
        }
    }

    /// Task that updates the list in response to messages from individual items.
    async fn task_update_from_todos(self) {
        while let Some(ev) = self.output_to_list.get().await {
            match ev {
                TodoItemMsg::Remove(id) => {
                    let patch = self
                        .inner
                        .visit(|items| {
                            items.iter().enumerate().find_map(|(index, item)| {
                                if item.id == id {
                                    Some(ListPatch::remove(index) as ListPatch<TodoItem>)
                                } else {
                                    None
                                }
                            })
                        })
                        .await
                        .expect("could not find item");
                    let _ = self.inner.patch(patch).await;
                }
                TodoItemMsg::Completion => {
                    self.inner.refresh().await;
                }
            }
        }
    }

    /// Task that each time there is an update to items, serializes the items to storage.
    async fn task_serialize(self) {
        let mut any_update = self.inner.stream();

        while let Some(_) = any_update.next().await {
            let store_items: Vec<_> = self
                .inner
                .read()
                .await
                .iter()
                .map(|item| crate::store::Item {
                    title: item.name.current().expect("could not read name"),
                    completed: item.complete.current().expect("could not read complete"),
                })
                .collect();
            crate::store::write_items(&store_items).expect("could not serialize items");
        }
    }

    /// Task that deserializes items from storage at startup and then creates a todo item for each
    async fn task_deserialize(self) {
        let items = crate::store::read_items().expect("could not deserialize items");
        for item in items.into_iter() {
            log::info!("read stored items: {:?}", item);
            self.push_new_todo(item.title, item.completed).await;
        }
    }

    pub fn viewbuilder(self) -> ViewBuilder {
        let captured_todo_input = Captured::<JsDom>::default();
        let captured_toggle_all_complete = Captured::<JsDom>::default();

        let input_filter_show = FanInput::<FilterShow>::default();

        let output_todo_input_change = Output::<JsDomEvent>::default();
        let output_toggle_all_clicked = Output::<()>::default();
        let output_clear_completed_clicked = Output::<()>::default();
        let output_window_onhashchange = Output::<JsDomEvent>::default();

        let builder = rsx! {
            section(id="todo_main", class="todoapp") {
                header(class = "header") {
                    h1() { "todos" }
                    input(
                        class = "new-todo",
                        id = "new-todo",
                        placeholder = "What needs to be done?",
                        on:change = output_todo_input_change.sink(),
                        capture:view = captured_todo_input.sink()
                    ) {}
                }
                section(
                    class = "main",
                    style:display = self
                        .stream_should_show_todo_list()
                        .map(should_show_bool_to_display_string)
                ) {
                    // This is the "check all as complete" toggle
                    input(
                        id = "toggle-all",
                        type = "checkbox",
                        class = "toggle-all",
                        capture:view = captured_toggle_all_complete.sink(),
                        on:click = output_toggle_all_clicked.sink().contra_map(|_:JsDomEvent| ())
                    ) {}
                    label(for_ = "toggle-all") { "Mark all as complete" }
                    ul(
                        class = "todo-list",
                        style:display = self
                            .stream_should_show_todo_list()
                            .map(should_show_bool_to_display_string),
                        patch:children = self.stream_of_viewbuilders()
                    ) {}
                }
                footer(
                    class = "footer",
                    style:display = self
                        .stream_should_show_todo_list()
                        .map(should_show_bool_to_display_string),
                ) {
                    span(class="todo-count") {
                        strong() {
                            {("", self.stream_number_of_items().map(num_items_to_user_string))}
                        }
                    }
                    ul(
                        class="filters",
                        window:hashchange = output_window_onhashchange.sink()
                    ) {
                        li() {
                            a(
                                href = "#/",
                                class = input_filter_show
                                    .stream()
                                    .map(|filt| filter_selected(filt, FilterShow::All))
                            ) { "All" }
                        }
                        li() {
                            a(
                                href = "#/active",
                                class = input_filter_show
                                    .stream()
                                    .map(|filt| filter_selected(filt, FilterShow::Active))
                            ) { "Active" }
                        }
                        li() {
                            a(
                                href = "#/completed",
                                class = input_filter_show
                                    .stream()
                                    .map(|filt| filter_selected(filt, FilterShow::Completed))
                            ) { "Completed" }
                        }
                    }
                    button(
                        class = "clear-completed",
                        style:display = self
                            .stream_should_show_complete_button()
                            .map(should_show_bool_to_display_string),
                        on:click = output_clear_completed_clicked.sink().contra_map(|_:JsDomEvent| ())
                    ) { "Clear completed" }
                }
            }
        };
        builder
            .with_task(self.clone().task_make_new_todos(output_todo_input_change))
            .with_task(self.clone().task_toggle_all(output_toggle_all_clicked))
            .with_task(
                self.clone()
                    .task_clear_completed(output_clear_completed_clicked),
            )
            .with_task(
                self.clone()
                    .task_filter_items(input_filter_show, output_window_onhashchange),
            )
            .with_task(self.clone().task_update_from_todos())
            .with_task(self.clone().task_serialize())
            .with_task(self.task_deserialize())
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

fn filter_selected(a: FilterShow, b: FilterShow) -> String {
    if a == b { "selected" } else { "" }.to_string()
}

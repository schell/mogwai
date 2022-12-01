use mogwai::{
    futures::stream::{FuturesOrdered, FuturesUnordered},
    prelude::*,
};
use std::iter::FromIterator;
use wasm_bindgen::JsCast;
use web_sys::{HashChangeEvent, HtmlInputElement};

use crate::{store, store::Item, utils};

pub mod item;
use item::Todo;

pub fn url_to_filter(url: String) -> Option<FilterShow> {
    let ndx = url.find('#').unwrap_or(0);
    let (_, hash) = url.split_at(ndx);
    match hash {
        "#/" => Some(FilterShow::All),
        "#/active" => Some(FilterShow::Active),
        "#/completed" => Some(FilterShow::Completed),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterShow {
    All,
    Completed,
    Active,
}

/// Messages sent from the view or from the [`App`] facade.
#[derive(Clone, Debug)]
enum AppLogic {
    SetFilter(FilterShow, Option<mpsc::Sender<()>>),
    NewTodo(String, bool),
    ChangedCompletion(usize, bool),
    ToggleCompleteAll,
    Remove(usize),
    RemoveCompleted,
}

#[derive(Clone)]
pub enum AppView {
    ShouldShowTodoList(bool),
    NumItems(usize),
    ShouldShowCompleteButton(bool),
    SelectedFilter(FilterShow),
}

/// App is a facade that communicates with the main logic loop by
/// relaying external function calls using enum messages.
pub struct App {
    tx_logic: broadcast::Sender<AppLogic>,
}

impl App {
    pub fn new() -> (App, ViewBuilder<JsDom>) {
        let (tx_logic, rx_logic) = broadcast::bounded(16);
        let (tx_view, rx_view) = broadcast::bounded(1);
        let (tx_todo_input, rx_todo_input) = mpsc::bounded(1);
        let (tx_toggle_input, rx_toggle_input) = mpsc::bounded(1);
        let (tx_patch_items, rx_patch_items) = mpsc::bounded(1);

        let component = view(
            tx_todo_input,
            tx_toggle_input,
            tx_logic.clone(),
            rx_view,
            rx_patch_items,
        )
        .with_task(logic(
            rx_logic,
            rx_todo_input,
            rx_toggle_input,
            tx_view,
            tx_patch_items,
        ));
        (App { tx_logic }, component)
    }

    pub async fn add_item(&self, item: Item) {
        self.tx_logic
            .broadcast(AppLogic::NewTodo(item.title, item.completed))
            .await
            .unwrap();
    }

    pub async fn filter(&self, fs: FilterShow) {
        let (tx, mut rx) = mpsc::bounded(1);
        self.tx_logic
            .broadcast(AppLogic::SetFilter(fs, Some(tx)))
            .await
            .unwrap();
        rx.next().await.unwrap();
    }
}

fn filter_selected(msg: AppView, show: FilterShow) -> Option<String> {
    match msg {
        AppView::SelectedFilter(msg_show) => Some(if msg_show == show {
            "selected".to_string()
        } else {
            "".to_string()
        }),
        _ => None,
    }
}

async fn are_all_complete(todos: impl Iterator<Item = &item::Todo>) -> bool {
    for todo in todos {
        if todo.is_done().await {
            continue;
        }
        return false;
    }
    true
}

async fn num_items_left(todos: impl Iterator<Item = &item::Todo>) -> usize {
    FuturesUnordered::from_iter(todos.map(Todo::is_done))
        .fold(0, |n, done| async move { n + if done { 1 } else { 0 } })
        .await
}

async fn logic(
    rx_logic: broadcast::Receiver<AppLogic>,
    mut recv_todo_input: mpsc::Receiver<JsDom>,
    mut recv_todo_toggle_input: mpsc::Receiver<JsDom>,
    tx_view: broadcast::Sender<AppView>,
    mut tx_item_patches: mpsc::Sender<ListPatch<ViewBuilder<JsDom>>>,
) {
    let todo_input = recv_todo_input.next().await.unwrap();
    let _ = mogwai::time::wait_secs(1.0).await;
    todo_input
        .visit_as(
            |i: &web_sys::HtmlElement| {
                i.focus().unwrap();
            },
            |_| {},
        )
        .unwrap();

    let todo_toggle_input = recv_todo_toggle_input.next().await.unwrap();

    let mut items: Vec<item::Todo> = vec![];
    let mut next_index = 0;
    let mut all_logic_sources = mogwai::futures::stream::select_all(vec![rx_logic.mogwai_stream()]);

    while let Some(msg) = all_logic_sources.next().await {
        let mut needs_check_complete = false;
        match msg {
            AppLogic::NewTodo(name, complete) => {
                let index = next_index;
                next_index += 1;
                // Create a new todo item and add it to our list of todos.
                let (todo, view_builder) = item::Todo::new(index, name.to_string());
                // Take the streams of updates from the todo and add them to our logic
                // sources.
                let was_removed = todo
                    .was_removed()
                    .map(move |_| AppLogic::Remove(index))
                    .mogwai_stream();
                all_logic_sources.push(was_removed);

                let has_changed_completion = todo
                    .has_changed_completion()
                    .map(move |complete| AppLogic::ChangedCompletion(index, complete))
                    .mogwai_stream();
                all_logic_sources.push(has_changed_completion);

                // Add the todo to communicate downstream later, and patch the view
                tx_item_patches
                    .send(ListPatch::push(view_builder))
                    .await
                    .unwrap();

                if complete {
                    todo.set_complete(true).await;
                }

                items.push(todo);

                todo_input.visit_as(
                    |i: &HtmlInputElement| i.set_value(""),
                    |i| i.set_attrib("value", Some("")).unwrap(),
                );

                tx_view
                    .broadcast(AppView::NumItems(items.len()))
                    .await
                    .unwrap();
                tx_view
                    .broadcast(AppView::ShouldShowTodoList(true))
                    .await
                    .unwrap();
                needs_check_complete = true;
            }
            AppLogic::SetFilter(show, may_tx) => {
                // Filter all the items, update the view, and then respond to the query.
                let filter_ops = mogwai::futures::stream::FuturesUnordered::from_iter(
                    items.iter().map(|todo| todo.filter(show.clone())),
                );
                let _ = filter_ops.collect::<Vec<_>>().await;

                tx_view
                    .broadcast(AppView::SelectedFilter(show.clone()))
                    .await
                    .unwrap();
                if let Some(mut tx) = may_tx {
                    tx.send(()).await.unwrap();
                }
            }
            AppLogic::ChangedCompletion(_index, _is_complete) => {
                let items_left = num_items_left(items.iter()).await;
                todo_toggle_input.visit_as(
                    |i: &HtmlInputElement| i.set_checked(items_left == 0),
                    |_| {},
                );
                tx_view
                    .broadcast(AppView::NumItems(items_left))
                    .await
                    .unwrap();
                needs_check_complete = true;
            }
            AppLogic::ToggleCompleteAll => {
                let should_complete = todo_toggle_input
                    .clone_as::<HtmlInputElement>()
                    .map(|el| el.checked())
                    .unwrap_or(false);
                for todo in items.iter() {
                    todo.set_complete(should_complete).await;
                }
                needs_check_complete = true;
            }
            AppLogic::Remove(index) => {
                let mut may_found_index = None;
                'remove_todo: for (todo, i) in items.iter().zip(0..) {
                    let todo_index = todo.index;
                    if todo_index == index {
                        // Send a patch to the view to remove the todo
                        tx_item_patches
                            .send(ListPatch::splice(i..=i, std::iter::empty()))
                            .await
                            .unwrap();
                        may_found_index = Some(i);
                        break 'remove_todo;
                    }
                }

                if let Some(i) = may_found_index {
                    let _ = items.remove(i);
                }

                if items.is_empty() {
                    // Update the toggle input checked state by hand
                    let checked = !are_all_complete(items.iter()).await;
                    if let Some(input) = todo_toggle_input.clone_as::<HtmlInputElement>() {
                        input.set_checked(checked);
                    }
                    tx_view
                        .broadcast(AppView::ShouldShowTodoList(false))
                        .await
                        .unwrap();
                }
                tx_view
                    .broadcast(AppView::NumItems(num_items_left(items.iter()).await))
                    .await
                    .unwrap();
                needs_check_complete = true;
            }
            AppLogic::RemoveCompleted => {
                let num_items_before = items.len();
                let mut to_remove = vec![];
                for (todo, i) in items.iter().zip(0..num_items_before).rev() {
                    if todo.is_done().await {
                        to_remove.push(i);
                        tx_item_patches
                            .send(ListPatch::splice(i..=i, std::iter::empty()))
                            .await
                            .unwrap();
                    }
                }
                to_remove.into_iter().for_each(|i| {
                    let _ = items.remove(i);
                });
                let checked = !are_all_complete(items.iter()).await;
                if let Some(input) = todo_toggle_input.clone_as::<HtmlInputElement>() {
                    input.set_checked(checked);
                }
                tx_view
                    .broadcast(AppView::NumItems(num_items_left(items.iter()).await))
                    .await
                    .unwrap();
                if items.is_empty() && num_items_before != 0 {
                    tx_view
                        .broadcast(AppView::ShouldShowTodoList(false))
                        .await
                        .unwrap();
                }
                needs_check_complete = true;
            }
        }

        // In any case, serialize the current todo items.
        let store_items = FuturesOrdered::from_iter(items.iter().map(Todo::as_item))
            .collect::<Vec<_>>()
            .await;
        store::write_items(store_items).expect("Could not store todos");
        // update the "clear completed" button if need be
        if needs_check_complete {
            let mut has_completed = false;
            for facade in items.iter() {
                if facade.is_done().await {
                    has_completed = true;
                    break;
                }
            }
            tx_view
                .broadcast(AppView::ShouldShowCompleteButton(has_completed))
                .await
                .unwrap();
        }
    }

    log::error!("leaving app logic");
}

fn todo_list_display(rx: &broadcast::Receiver<AppView>) -> impl Stream<Item = String> {
    rx.clone().filter_map(|msg| async move {
        match msg {
            AppView::ShouldShowTodoList(should) => {
                Some(if should { "block" } else { "none" }.to_string())
            }
            _ => None,
        }
    })
}

fn view(
    send_todo_input: mpsc::Sender<JsDom>,
    send_completion_toggle_input: mpsc::Sender<JsDom>,
    tx: broadcast::Sender<AppLogic>,
    rx: broadcast::Receiver<AppView>,
    item_children: impl MogwaiStream<ListPatch<ViewBuilder<JsDom>>>,
) -> ViewBuilder<JsDom> {
    html! {
        <section id="todo_main" class="todoapp">
            <header class="header">
                <h1>"todos"</h1>
                <input
                 class="new-todo" id="new-todo" placeholder="What needs to be done?"
                 on:change = tx.clone().with_flat_map(|ev: JsDomEvent| {
                     let todo_name =
                         utils::event_input_value(ev).expect("event input value");
                     if todo_name.is_empty() {
                         Either::Left(stream::empty())
                     } else {
                         Either::Right(stream::once(async move {Ok(AppLogic::NewTodo(todo_name, false))}))
                     }
                 })
                 capture:view=send_todo_input>
                </input>
            </header>
            <section class="main" style:display=("none", todo_list_display(&rx))>
                // This is the "check all as complete" toggle
                <input
                 id="toggle-all"
                 type="checkbox"
                 class="toggle-all"
                 capture:view=send_completion_toggle_input
                 on:click=tx.clone().contra_map(|_| AppLogic::ToggleCompleteAll)>
                </input>
                <label for="toggle-all">"Mark all as complete"</label>
                <ul class="todo-list"
                 style:display=("none", todo_list_display(&rx))
                 patch:children=item_children>
                </ul>
            </section>
            <footer class="footer" style:display=("none", todo_list_display(&rx))>
                <span class="todo-count">
                    <strong>
                        {(
                            "0 items left",
                            rx.clone().filter_map(|msg| async move { match msg {
                                AppView::NumItems(n) => {
                                    let items = if n == 1 { "item" } else { "items" };
                                    Some(format!("{} {} left", n, items))
                                }
                                _ => None,
                            }})
                        )}
                    </strong>
                </span>
                <ul class="filters"
                    window:hashchange=
                        tx.clone().with_flat_map(|ev: JsDomEvent| {
                            let ev: web_sys::Event = ev.browser_event().unwrap();
                            let ev: HashChangeEvent =
                                ev.dyn_into::<HashChangeEvent>().expect("not hash event");
                            let url = ev.new_url();
                            if let Some(filter) = url_to_filter(url) {
                                Either::Left(stream::once(async move {Ok(AppLogic::SetFilter(filter, None))}))
                            } else {
                                Either::Right(stream::empty())
                            }
                        })>
                    <li>
                        <a href="#/"
                         class=rx.clone().filter_map(|msg| async move {filter_selected(msg, FilterShow::All)})>
                            "All"
                        </a>
                    </li>
                    <li>
                        <a href="#/active"
                         class=rx.clone().filter_map(|msg| async move {filter_selected(msg, FilterShow::Active)})>
                            "Active"
                        </a>
                    </li>
                    <li>
                        <a href="#/completed"
                        class=rx.clone().filter_map(|msg| async move {filter_selected(msg, FilterShow::Completed)})>
                            "Completed"
                        </a>
                    </li>
                </ul>
                <button
                    class="clear-completed"
                    style:display=
                        (
                            "none",
                            rx.clone().filter_map(|msg| async move { match msg {
                                AppView::ShouldShowCompleteButton(should) => {
                                    Some(if should { "block" } else { "none" }.to_string())
                                }
                                _ => None,
                            }})
                        )
                    on:click=tx.contra_map(|_: JsDomEvent| AppLogic::RemoveCompleted)>
                    "Clear completed"
                </button>
            </footer>
        </section>
    }
}

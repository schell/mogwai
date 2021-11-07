use mogwai::{
    futures::stream::{FuturesOrdered, FuturesUnordered},
    prelude::*,
};
use std::iter::FromIterator;
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
#[derive(Clone)]
enum AppLogic {
    SetFilter(FilterShow, Option<mpmc::Sender<()>>),
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
    pub fn new() -> (App, Component<Dom>) {
        let (tx_logic, rx_logic) = broadcast::bounded(1);
        let (tx_view, rx_view) = broadcast::bounded(1);
        let (tx_todo_input, rx_todo_input) = mpmc::bounded(1);
        let (tx_toggle_input, rx_toggle_input) = mpmc::bounded(1);
        let (tx_patch_items, rx_patch_items) = mpmc::bounded(1);

        let component = Component::from(view(
            tx_todo_input,
            tx_toggle_input,
            tx_logic.clone(),
            rx_view,
            rx_patch_items,
        )).with_logic(logic(
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
        let (tx, rx) = mpmc::bounded(1);
        self.tx_logic
            .broadcast(AppLogic::SetFilter(fs, Some(tx)))
            .await
            .unwrap();
        rx.recv().await.unwrap();
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

async fn are_any_complete(todos: impl Iterator<Item = &item::Todo>) -> bool {
    for todo in todos {
        if todo.is_done().await {
            return true;
        }
    }
    return false;
}

async fn maybe_update_completed(
    todos: impl Iterator<Item = &item::Todo>,
    local_has_completed: &mut bool,
    tx: &mut broadcast::Sender<AppView>,
) {
    let has_completed = are_any_complete(todos).await;
    if *local_has_completed != has_completed {
        *local_has_completed = has_completed;
        tx.broadcast(AppView::ShouldShowCompleteButton(has_completed))
            .await
            .unwrap();
    }
}

async fn num_items_left(todos: impl Iterator<Item = &item::Todo>) -> usize {
    FuturesUnordered::from_iter(todos.map(Todo::is_done))
        .fold(0, |n, done| async move { n + if done { 1 } else { 0 } })
        .await
}

async fn logic(
    rx_logic: broadcast::Receiver<AppLogic>,
    mut recv_todo_input: mpmc::Receiver<Dom>,
    mut recv_todo_toggle_input: mpmc::Receiver<Dom>,
    mut tx_view: broadcast::Sender<AppView>,
    tx_item_patches: mpmc::Sender<ListPatch<ViewBuilder<Dom>>>,
) {
    let todo_input = recv_todo_input.next().await.unwrap();
    let _ = mogwai::time::wait_approx(1.0).await;
    todo_input.visit_as::<HtmlInputElement, _, _>(
        |i| {
            i.focus().unwrap();
        },
        |_| {},
    );

    let todo_toggle_input = recv_todo_toggle_input.next().await.unwrap();

    let mut items: Vec<item::Todo> = vec![];
    let mut has_completed = false;
    let mut next_index = 0;
    let mut all_logic_sources = mogwai::futures::stream::select_all(vec![rx_logic.boxed()]);

    loop {
        match all_logic_sources.next().await {
            Some(AppLogic::NewTodo(name, complete)) => {
                let index = next_index;
                next_index += 1;
                // Create a new todo item and add it to our list of todos.
                let (todo, view_builder) = item::Todo::new(index, name.to_string());
                // Take the streams of updates from the todo and add them to our logic
                // sources.
                let was_removed = todo
                    .was_removed()
                    .map(move |_| AppLogic::Remove(index))
                    .boxed();
                all_logic_sources.push(was_removed);
                let has_changed_completion = todo
                    .has_changed_completion()
                    .map(move |complete| {
                        log::warn!("got completion");
                        AppLogic::ChangedCompletion(index, complete)
                    })
                    .boxed();
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

                todo_input.visit_as::<HtmlInputElement, _, _>(
                    |i| i.set_value(""),
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

                maybe_update_completed(items.iter(), &mut has_completed, &mut tx_view).await;
            }
            Some(AppLogic::SetFilter(show, may_tx)) => {
                // Filter all the items, update the view, and then respond to the query.
                let filter_ops = mogwai::futures::stream::FuturesUnordered::from_iter(
                    items.iter().map(|todo| todo.filter(show.clone())),
                );
                let _ = filter_ops.collect::<Vec<_>>().await;

                tx_view
                    .broadcast(AppView::SelectedFilter(show.clone()))
                    .await
                    .unwrap();
                if let Some(tx) = may_tx {
                    tx.send(()).await.unwrap();
                }
            }
            Some(AppLogic::ChangedCompletion(_index, _is_complete)) => {
                let items_left = num_items_left(items.iter()).await;
                todo_toggle_input.visit_as(
                    |i: &HtmlInputElement| i.set_checked(items_left == 0),
                    |_| {},
                );
                tx_view
                    .broadcast(AppView::NumItems(items_left))
                    .await
                    .unwrap();
                maybe_update_completed(items.iter(), &mut &mut has_completed, &mut tx_view).await;
            }
            Some(AppLogic::ToggleCompleteAll) => {
                let should_complete = todo_toggle_input
                    .clone_as::<HtmlInputElement>()
                    .map(|el| el.checked())
                    .unwrap_or(false);
                for todo in items.iter() {
                    todo.set_complete(should_complete).await;
                }
                maybe_update_completed(items.iter(), &mut &mut has_completed, &mut tx_view).await;
            }
            Some(AppLogic::Remove(index)) => {
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
                    tx_view.broadcast(AppView::ShouldShowTodoList(false)).await.unwrap();
                }
                tx_view
                    .broadcast(AppView::NumItems(num_items_left(items.iter()).await))
                    .await
                    .unwrap();
                maybe_update_completed(items.iter(), &mut has_completed, &mut tx_view).await;
            }
            Some(AppLogic::RemoveCompleted) => {
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
                maybe_update_completed(items.iter(), &mut has_completed, &mut tx_view).await;
                if items.is_empty() && num_items_before != 0 {
                    tx_view
                        .broadcast(AppView::ShouldShowTodoList(false))
                        .await
                        .unwrap();
                }
            }
            None => break,
        }

        // In any case, serialize the current todo items.
        let store_items = FuturesOrdered::from_iter(items.iter().map(Todo::as_item))
            .collect::<Vec<_>>()
            .await;
        store::write_items(store_items).expect("Could not store todos");
        log::info!("stored todos");
    }
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
    send_todo_input: mpmc::Sender<Dom>,
    send_completion_toggle_input: mpmc::Sender<Dom>,
    tx: broadcast::Sender<AppLogic>,
    rx: broadcast::Receiver<AppView>,
    item_children: impl Streamable<ListPatch<ViewBuilder<Dom>>>,
) -> ViewBuilder<Dom> {
    builder! {
        <section id="todo_main" class="todoapp">
            <header class="header">
                <h1>"todos"</h1>
                <input
                 class="new-todo" id="new-todo" placeholder="What needs to be done?"
                 on:change = tx.sink().with_flat_map(|ev: Event| {
                     let todo_name =
                         utils::event_input_value(&ev).expect("event input value");
                     if todo_name.is_empty() {
                         Either::Left(stream::empty())
                     } else {
                         Either::Right(stream::once(async move {Ok(AppLogic::NewTodo(todo_name, false))}))
                     }
                 })
                 post:build=move |dom: &mut Dom| {
                     send_todo_input.try_send(dom.clone()).unwrap();
                 }>
                </input>
            </header>
            <section class="main" style:display=("none", todo_list_display(&rx))>
                // This is the "check all as complete" toggle
                <input
                 id="toggle-all"
                 type="checkbox"
                 class="toggle-all"
                 post:build=move |dom: &mut Dom| {
                     send_completion_toggle_input.try_send(dom.clone()).unwrap();
                 }
                 on:click=tx.sink().with(|_| async{Ok(AppLogic::ToggleCompleteAll)})>
                </input>
                <label for="toggle-all">"Mark all as complete"</label>
                <ul class="todo-list"
                 style:display=("none", todo_list_display(&rx))
                    //post:build=tx.contra_map(|el: &HtmlElement| In::TodoListUl(el.clone()))>
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
                        tx.sink().with_flat_map(|ev: Event| {
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
                    on:click=tx.sink().with(|_: Event| async {Ok(AppLogic::RemoveCompleted)})>
                    "Clear completed"
                </button>
            </footer>
        </section>
    }
}

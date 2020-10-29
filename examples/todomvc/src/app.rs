use mogwai::prelude::*;
use web_sys::{HashChangeEvent, HtmlInputElement};

use super::{store, store::Item, utils};

mod item;
use item::{Todo, TodoIn, TodoOut};

#[derive(Clone, Debug, PartialEq)]
pub enum FilterShow {
    All,
    Completed,
    Active,
}

#[derive(Clone, Debug)]
pub enum In {
    NewTodo(String, bool),
    NewTodoInput(HtmlInputElement),
    Filter(FilterShow),
    CompletionToggleInput(HtmlInputElement),
    ChangedCompletion(usize, bool),
    ToggleCompleteAll,
    Remove(usize),
    RemoveCompleted,
}

#[derive(Clone)]
pub enum Out {
    ShouldShowTodoList(bool),
    NumItems(usize),
    ShouldShowCompleteButton(bool),
    SelectedFilter(FilterShow),
    PatchTodos(Patch<View<HtmlElement>>),
}

pub struct App {
    next_index: usize,
    todos: Vec<Gizmo<Todo>>,
    todo_input: Option<HtmlInputElement>,
    todo_toggle_input: Option<HtmlInputElement>,
    has_completed: bool,
}

impl App {
    pub fn new() -> App {
        App {
            next_index: 0,
            todos: vec![],
            todo_input: None,
            todo_toggle_input: None,
            has_completed: false,
        }
    }

    fn num_items_left(&self) -> usize {
        self.todos.iter().fold(0, |n, todo| {
            n + todo.with_state(|t| if t.is_done { 0 } else { 1 })
        })
    }

    fn are_any_complete(&self) -> bool {
        for todo in self.todos.iter() {
            if todo.with_state(|t| t.is_done) {
                return true;
            }
        }
        return false;
    }

    fn are_all_complete(&self) -> bool {
        self.todos.iter().fold(true, |complete, todo| {
            complete && todo.with_state(|t| t.is_done)
        })
    }

    fn items(&self) -> Vec<Item> {
        self.todos
            .iter()
            .map(|component| {
                component.with_state(|todo| Item {
                    title: todo.name.clone(),
                    completed: todo.is_done,
                })
            })
            .collect()
    }

    pub fn url_to_filter_msg(url: String) -> Option<In> {
        let ndx = url.find('#').unwrap_or(0);
        let (_, hash) = url.split_at(ndx);
        match hash {
            "#/" => Some(In::Filter(FilterShow::All)),
            "#/active" => Some(In::Filter(FilterShow::Active)),
            "#/completed" => Some(In::Filter(FilterShow::Completed)),
            _ => None,
        }
    }

    fn filter_selected(msg: &Out, show: FilterShow) -> Option<String> {
        match msg {
            Out::SelectedFilter(msg_show) => Some(if *msg_show == show {
                "selected".to_string()
            } else {
                "".to_string()
            }),
            _ => None,
        }
    }

    fn maybe_update_completed(&mut self, tx: &Transmitter<Out>) {
        let has_completed = self.are_any_complete();
        if self.has_completed != has_completed {
            self.has_completed = has_completed;
            tx.send(&Out::ShouldShowCompleteButton(self.are_any_complete()));
        }
    }

    fn clear_todo_input(&mut self) {
        if let Some(input) = &self.todo_input {
            input.set_value("");
        }
    }
}

impl Component for App {
    type ModelMsg = In;
    type ViewMsg = Out;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &In, tx_view: &Transmitter<Out>, sub: &Subscriber<In>) {
        match msg {
            In::NewTodo(name, complete) => {
                let index = self.next_index;
                // Turn the new todo into a gizmo, wire it up and spawn a viewbuilder, turning it into a view
                // and sending to patch our todo list
                let component = Gizmo::new(Todo::new(index, name.to_string()));
                // Subscribe to some of its view messages
                sub.subscribe_filter_map(&component.recv, move |todo_out_msg| match todo_out_msg {
                    TodoOut::UpdateEditComplete(_, is_complete) => {
                        Some(In::ChangedCompletion(index, *is_complete))
                    }
                    TodoOut::Remove => Some(In::Remove(index)),
                    _ => None,
                });
                if *complete {
                    component.send(&TodoIn::SetCompletion(true));
                }

                // Add the gizmo's view to the app's view
                tx_view.send(&Out::PatchTodos(Patch::PushBack {
                    value: View::from(component.view_builder()),
                }));

                // Store the gizmo so we can update it later
                self.todos.push(component);
                self.next_index += 1;

                self.clear_todo_input();

                tx_view.send(&Out::NumItems(self.todos.len()));
                tx_view.send(&Out::ShouldShowTodoList(true));
            }
            In::NewTodoInput(input) => {
                self.todo_input = Some(input.clone());
                let input = input.clone();
                timeout(0, move || {
                    input.focus().expect("focus");
                    // Never reschedule the timeout
                    false
                });
            }
            In::Filter(show) => {
                self.todos.iter().for_each(|component| {
                    let is_done = component.with_state(|t| t.is_done);
                    let is_visible = *show == FilterShow::All
                        || (*show == FilterShow::Completed && is_done)
                        || (*show == FilterShow::Active && !is_done);
                    component.send(&TodoIn::SetVisible(is_visible));
                });
                tx_view.send(&Out::SelectedFilter(show.clone()));
            }
            In::CompletionToggleInput(input) => {
                self.todo_toggle_input = Some(input.clone());
                self.maybe_update_completed(tx_view);
            }
            In::ChangedCompletion(_index, _is_complete) => {
                let items_left = self.num_items_left();
                self.todo_toggle_input
                    .iter()
                    .for_each(|input| input.set_checked(items_left == 0));
                tx_view.send(&Out::NumItems(items_left));
                self.maybe_update_completed(tx_view);
            }
            In::ToggleCompleteAll => {
                let input = self.todo_toggle_input.as_ref().expect("toggle input");

                let should_complete = input.checked();
                for todo in self.todos.iter_mut() {
                    todo.send(&TodoIn::SetCompletion(should_complete));
                }
            }
            In::Remove(index) => {
                let mut may_found_index = None;
                'remove_todo: for (todo, i) in self.todos.iter().zip(0..) {
                    let todo_index = todo.with_state(|t| t.index);
                    if todo_index == *index {
                        // Send a patch to the view to remove the todo
                        tx_view.send(&Out::PatchTodos(Patch::Remove { index: i }));
                        may_found_index = Some(i);
                        break 'remove_todo;
                    }
                }

                if let Some(i) = may_found_index {
                    let _ = self.todos.remove(i);
                }

                if self.todos.len() == 0 {
                    // Update the toggle input checked state by hand
                    if let Some(input) = self.todo_toggle_input.as_ref() {
                        input.set_checked(!self.are_all_complete());
                    }
                    tx_view.send(&Out::ShouldShowTodoList(false));
                }
                tx_view.send(&Out::NumItems(self.num_items_left()));
                self.maybe_update_completed(tx_view);
            }
            In::RemoveCompleted => {
                let num_items_before = self.todos.len();
                let to_remove = self
                    .todos
                    .iter()
                    .zip(0..self.todos.len())
                    .rev()
                    .filter_map(|(todo, i)| {
                        if todo.with_state(|t| t.is_done) {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                to_remove.into_iter().for_each(|i| {
                    let _ = self.todos.remove(i);
                    tx_view.send(&Out::PatchTodos(Patch::Remove { index: i }));
                });
                self.todo_toggle_input
                    .iter()
                    .for_each(|input| input.set_checked(!self.are_all_complete()));
                tx_view.send(&Out::NumItems(self.num_items_left()));
                self.maybe_update_completed(tx_view);
                if self.todos.len() == 0 && num_items_before != 0 {
                    tx_view.send(&Out::ShouldShowTodoList(false));
                }
            }
        };

        // In any case, serialize the current todo items.
        let items = self.items();
        store::write_items(items).expect("Could not store todos");
    }

    fn view(&self, tx: &Transmitter<In>, rx: &Receiver<Out>) -> ViewBuilder<HtmlElement> {
        let rx_display = rx.branch_filter_map(|msg| match msg {
            Out::ShouldShowTodoList(should) => {
                Some(if *should { "block" } else { "none" }.to_string())
            }
            _ => None,
        });

        builder! {
            <section id="todo_main" class="todoapp">
                <header class="header">
                    <h1>"todos"</h1>
                    <input cast:type=web_sys::HtmlInputElement
                        class="new-todo" id="new-todo" placeholder="What needs to be done?"
                        on:change=
                            tx.contra_filter_map(|ev: &Event| {
                                let todo_name =
                                    utils::event_input_value(ev).expect("event input value");
                                if todo_name.is_empty() {
                                    None
                                } else {
                                    Some(In::NewTodo(todo_name, false))
                                }
                            })
                        post:build=
                            tx.contra_map(|el: &HtmlInputElement| In::NewTodoInput(el.clone()))>
                    </input>
                </header>
                <section class="main" style:display=("none", rx_display.branch())>
                    // This is the "check all as complete" toggle
                    <input cast:type=web_sys::HtmlInputElement
                        id="toggle-all"
                        type="checkbox"
                        class="toggle-all"
                        post:build=
                            tx.contra_map(|el: &HtmlInputElement| {
                                In::CompletionToggleInput(el.clone())
                            })
                        on:click=tx.contra_map(|_| In::ToggleCompleteAll)>
                    </input>
                    <label for="toggle-all">"Mark all as complete"</label>
                    <ul class="todo-list"
                        style:display=("none", rx_display.branch())
                        //post:build=tx.contra_map(|el: &HtmlElement| In::TodoListUl(el.clone()))>
                        patch:children=rx.branch_filter_map(|msg| match msg {
                            Out::PatchTodos(p) => Some(p.clone()),
                            _ => None
                        })>
                    </ul>
                </section>
                <footer class="footer" style:display=("none", rx_display)>
                    <span class="todo-count">
                        <strong>
                            {(
                                "0 items left",
                                rx.branch_filter_map(|msg| match msg {
                                    Out::NumItems(n) => {
                                        let items = if *n == 1 { "item" } else { "items" };
                                        Some(format!("{} {} left", n, items))
                                    }
                                    _ => None,
                                })
                            )}
                        </strong>
                    </span>
                    <ul class="filters"
                        window:hashchange=
                            tx.contra_filter_map(|ev: &Event| {
                                let ev: &HashChangeEvent =
                                    ev.dyn_ref::<HashChangeEvent>().expect("not hash event");
                                let url = ev.new_url();
                                App::url_to_filter_msg(url)
                            })>
                        <li>
                            <a href="#/" class=rx.branch_filter_map(|msg| App::filter_selected(msg, FilterShow::All))>"All"</a>
                        </li>
                        <li>
                            <a href="#/active" class=rx.branch_filter_map(|msg| App::filter_selected(msg, FilterShow::Active))>"Active"</a>
                        </li>
                        <li>
                            <a href="#/completed" class=rx.branch_filter_map(|msg| App::filter_selected(msg, FilterShow::Completed))>"Completed"</a>
                        </li>
                    </ul>
                    <button
                        class="clear-completed"
                        style:display=
                            (
                                "none",
                                rx.branch_filter_map(|msg| match msg {
                                    Out::ShouldShowCompleteButton(should) => {
                                        Some(if *should { "block" } else { "none" }.to_string())
                                    }
                                    _ => None,
                                })
                            )
                        on:click=tx.contra_map(|_: &Event| In::RemoveCompleted)>
                        "Clear completed"
                    </button>
                </footer>
            </section>
        }
    }
}

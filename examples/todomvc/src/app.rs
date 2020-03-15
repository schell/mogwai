use mogwai::prelude::*;
use web_sys::HashChangeEvent;

use super::store;
use super::store::Item;
use super::utils;

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
  TodoListUl(HtmlElement),
  Remove(usize),
  RemoveCompleted,
}


#[derive(Clone)]
pub enum Out {
  ClearNewTodoInput,
  ShouldShowTodoList(bool),
  NumItems(usize),
  ShouldShowCompleteButton(bool),
  SelectedFilter(FilterShow),
}


pub struct App {
  next_index: usize,
  todos: Vec<GizmoComponent<Todo>>,
  todo_input: Option<HtmlInputElement>,
  todo_toggle_input: Option<HtmlInputElement>,
  todo_list_ul: Option<HtmlElement>,
  has_completed: bool
}


impl App {
  pub fn new() -> App {
    App {
      next_index: 0,
      todos: vec![],
      todo_input: None,
      todo_toggle_input: None,
      todo_list_ul: None,
      has_completed: false
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
    return false
  }

  fn are_all_complete(&self) -> bool {
    self.todos.iter().fold(true, |complete, todo| {
      complete && todo.with_state(|t| t.is_done)
    })
  }

  fn items(&self) -> Vec<Item> {
    self
      .todos
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
      Out::SelectedFilter(msg_show) => Some(
        if *msg_show == show {
          "selected".to_string()
        } else {
          "".to_string()
        },
      ),
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
}


impl Component for App {
  type ModelMsg = In;
  type ViewMsg = Out;
  type DomNode = HtmlElement;

  fn update(
    &mut self,
    msg: &In,
    tx_view: &Transmitter<Out>,
    sub: &Subscriber<In>,
  ) {
    match msg {
      In::NewTodo(name, complete) => {
        let index = self.next_index;
        // Turn the new todo into a sub-component.
        let mut component = Todo::new(index, name.to_string()).into_component();
        // Subscribe to some of its view messages
        sub.subscribe_filter_map(&component.recv, move |todo_out_msg| {
          match todo_out_msg {
            TodoOut::UpdateEditComplete(_, is_complete) => {
              Some(In::ChangedCompletion(index, *is_complete))
            }
            TodoOut::Remove => Some(In::Remove(index)),
            _ => None,
          }
        });
        if *complete {
          component.update(&TodoIn::SetCompletion(true));
        }
        // If we have a ul, add the component to it.
        if let Some(ul) = self.todo_list_ul.as_ref() {
          let _ = ul.append_child(&component);
        }
        self.todos.push(component);
        self.next_index += 1;

        tx_view.send(&Out::ClearNewTodoInput);
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
        self.todos.iter_mut().for_each(|component| {
          let is_done = component.with_state(|t| t.is_done);
          let is_visible = *show == FilterShow::All
            || (*show == FilterShow::Completed && is_done)
            || (*show == FilterShow::Active && !is_done);
          component.update(&TodoIn::SetVisible(is_visible));
        });
        tx_view.send(&Out::SelectedFilter(show.clone()));
      }
      In::CompletionToggleInput(input) => {
        self.todo_toggle_input = Some(input.clone());
        self.maybe_update_completed(tx_view);
      }
      In::ChangedCompletion(_index, _is_complete) => {
        let items_left = self.num_items_left();
        self
          .todo_toggle_input
          .iter()
          .for_each(|input| input.set_checked(items_left == 0));
        tx_view.send(&Out::NumItems(items_left));
        self.maybe_update_completed(tx_view);
      }
      In::ToggleCompleteAll => {
        let input = self.todo_toggle_input.as_ref().expect("toggle input");

        let should_complete = input.checked();
        for todo in self.todos.iter_mut() {
          todo.update(&TodoIn::SetCompletion(should_complete));
        }
      }
      In::TodoListUl(ul) => {
        self.todo_list_ul = Some(ul.clone());
        // If we have todos already created (from local storage), add them to
        // the ul.
        self
          .todos
          .iter()
          .for_each(|component| {
            let _ = ul.append_child(&component);
          });
      }
      In::Remove(index) => {
        // Removing the gizmo drops its shared state, transmitters and receivers.
        // This causes its Drop implementation to run, which removes its
        // html_element from the parent.
        self.todos.retain(|todo| {
          let keep = todo.with_state(|t| t.index != *index);
          if !keep {
            if let Some(parent) = todo.dom_ref().parent_element() {
              let _ = parent.remove_child(todo.dom_ref());
            }
          }
          keep
        });

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
        self.todos.retain(|todo| todo.with_state(|t| !t.is_done));
        self
          .todo_toggle_input
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

  fn view(&self, tx: Transmitter<In>, rx: Receiver<Out>) -> Gizmo<HtmlElement> {
    let rx_display =
      rx.branch_filter_map(|msg| match msg {
      Out::ShouldShowTodoList(should) => {
        Some(if *should { "block" } else { "none" }.to_string())
      }
      _ => None,
    });

    section()
      .class("todoapp")
      .with(
        header().class("header").with(h1().text("todos")).with(
          input()
            .class("new-todo")
            .attribute("id", "new-todo")
            .attribute("placeholder", "What needs to be done?")
            .tx_on(
              "change",
              tx.contra_filter_map(|ev: &Event| {
                let todo_name =
                  utils::event_input_value(ev).expect("event input value");
                if todo_name.is_empty() {
                  None
                } else {
                  Some(In::NewTodo(todo_name, false))
                }
              }),
            )
            .rx_value(
              "",
              rx.branch_filter_map(|msg| match msg {
                Out::ClearNewTodoInput => Some("".to_string()),
                _ => None,
              }),
            )
            .tx_post_build(tx.contra_map(|el: &HtmlInputElement| {
              In::NewTodoInput(el.clone())
            })),
        ),
      )
      .with(
        section()
          .class("main")
          .rx_style("display", "none", rx_display.branch())
          .with(
            // This is the "check all as complete" toggle
            input()
              .attribute("id", "toggle-all")
              .attribute("type", "checkbox")
              .class("toggle-all")
              .tx_post_build(tx.contra_map(|el: &HtmlInputElement| {
                In::CompletionToggleInput(el.clone())
              }))
              .tx_on("click", tx.contra_map(|_| In::ToggleCompleteAll)),
          )
          .with(
            label()
              .attribute("for", "toggle-all")
              .text("Mark all as complete"),
          )
          .with(
            ul()
              .class("todo-list")
              .rx_style("display", "none", rx_display.branch())
              .tx_post_build(
                tx.contra_map(|el: &HtmlElement| In::TodoListUl(el.clone())),
              ),
          ),
      )
      .with(
        footer()
          .class("footer")
          .rx_style("display", "none", rx_display)
          .with(span().class("todo-count").with(strong().rx_text(
            "0 items left",
            rx.branch_filter_map(|msg| match msg {
              Out::NumItems(n) => {
                let items = if *n == 1 { "item" } else { "items" };
                Some(format!("{} {} left", n, items))
              }
              _ => None,
            }),
          )))
          .with(
            ul()
              .class("filters")
              .with(
                li().with(
                  a()
                    .rx_class(
                      "",
                      rx.branch_filter_map(|msg| {
                        App::filter_selected(msg, FilterShow::All)
                      }),
                    )
                    .attribute("href", "#/")
                    .text("All"),
                ),
              )
              .with(
                li().with(
                  a()
                    .rx_class(
                      "",
                      rx.branch_filter_map(|msg| {
                        App::filter_selected(msg, FilterShow::Active)
                      }),
                    )
                    .attribute("href", "#/active")
                    .text("Active"),
                ),
              )
              .with(
                li().with(
                  a()
                    .rx_class(
                      "",
                      rx.branch_filter_map(|msg| {
                        App::filter_selected(msg, FilterShow::Completed)
                      }),
                    )
                    .attribute("href", "#/completed")
                    .text("Completed"),
                ),
              )
              .tx_on_window(
                "hashchange",
                tx.contra_filter_map(|ev: &Event| {
                  let ev: &HashChangeEvent =
                    ev
                    .dyn_ref::<HashChangeEvent>()
                    .expect("not hash event");
                  let url = ev.new_url();
                  App::url_to_filter_msg(url)
                }),
              ),
          )
          .with(
            button()
              .class("clear-completed")
              .text("Clear completed")
              .rx_style(
                "display",
                "none",
                rx.branch_filter_map(|msg| match msg {
                  Out::ShouldShowCompleteButton(should) => {
                    Some(if *should { "block" } else { "none" }.to_string())
                  }
                  _ => None,
                }),
              )
              .tx_on("click", tx.contra_map(|_: &Event| In::RemoveCompleted)),
          ),
      )
  }
}

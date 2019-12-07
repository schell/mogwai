use mogwai::prelude::*;
use web_sys::KeyboardEvent;

use super::utils;


#[derive(Clone)]
pub struct Todo {
  pub index: usize,
  pub is_done: bool,
  pub name: String,
  is_editing: bool,
  edit_input: Option<HtmlInputElement>,
  toggle_input: Option<HtmlInputElement>,
}


impl Todo {
  pub fn new(index: usize, name: String) -> Todo {
    Todo {
      index,
      name,
      is_done: false,
      is_editing: false,
      edit_input: None,
      toggle_input: None,
    }
  }
}


pub enum TodoIn {
  CompletionToggleInput(HtmlElement),
  EditInput(HtmlElement),
  ToggleCompletion,
  SetCompletion(bool),
  StartEditing,
  StopEditing(Option<Event>),
  SetVisible(bool),
  Remove
}


#[derive(Clone)]
pub enum TodoOut {
  UpdateEditComplete(bool, bool),
  SetName(String),
  SetVisible(bool),
  Remove
}


impl TodoOut {
  fn as_list_class(&self) -> Option<String> {
    match self {
      TodoOut::UpdateEditComplete(editing, completed) => {
        Some(
          if *editing {
            "editing"
          } else if *completed {
            "completed"
          } else {
            ""
          }.to_string()
        )
      }
      _ => { None }
    }
  }
}


impl Component for Todo {
  type ModelMsg = TodoIn;
  type ViewMsg = TodoOut;

  fn update(&mut self, msg: &TodoIn, tx_view: &Transmitter<TodoOut>, _: &Subscriber<TodoIn>) {
    match msg {
      TodoIn::SetVisible(visible) => {
        tx_view.send(&TodoOut::SetVisible(*visible));
      }
      TodoIn::CompletionToggleInput(el) => {
        self.toggle_input = Some(
          el.clone()
            .dyn_into::<HtmlInputElement>()
            .expect("Todo toggle completion input is not an input")
        );
      }
      TodoIn::EditInput(el) => {
        self.edit_input = Some(
          el.clone()
            .dyn_into::<HtmlInputElement>()
            .expect("Todo edit input is not an input")
        );
      }
      TodoIn::ToggleCompletion => {
        self.is_done = !self.is_done;
        tx_view.send(&TodoOut::UpdateEditComplete(self.is_editing, self.is_done));
      }
      TodoIn::SetCompletion(completed) => {
        self.is_done = *completed;
        self
          .toggle_input
          .iter()
          .for_each(|input| input.set_checked(*completed));
        tx_view.send(&TodoOut::UpdateEditComplete(self.is_editing, self.is_done));
      }
      TodoIn::StartEditing => {
        self.is_editing = true;
        let input =
          self
          .edit_input
          .as_ref()
          .unwrap()
          .clone();
        timeout(1, move || {
          input
            .focus()
            .unwrap();
          false
        });
        tx_view.send(&TodoOut::UpdateEditComplete(self.is_editing, self.is_done));
      }
      TodoIn::StopEditing(may_ev) => {
        self.is_editing = false;

        let input:&HtmlInputElement =
          self
          .edit_input
          .as_ref()
          .unwrap();

        if let Some(ev) = may_ev {
          // This came from a key event
          let kev =
            ev
            .dyn_ref::<KeyboardEvent>()
            .unwrap();
          let key =
            kev.key();
          if key == "Enter" {
            utils::input_value(input)
              .into_iter()
              .for_each(|name| self.name = name);
          } else if key == "Escape" {
            self
              .edit_input
              .iter()
              .for_each(|input| input.set_value(&self.name));
          }
        } else {
          // This came from an input change event
          utils::input_value(input)
            .into_iter()
            .for_each(|name| self.name = name);
        }
        tx_view.send(&TodoOut::SetName(self.name.clone()));
        tx_view.send(&TodoOut::UpdateEditComplete(self.is_editing, self.is_done));
      }
      TodoIn::Remove => {
        // A todo cannot remove itself - its gizmo is owned by the parent App.
        // So we'll fire out a TodoOut::Remove and let App's update function
        // handle that.
        tx_view.send(&TodoOut::Remove);
      }
    }
  }

  fn builder(&self, tx: Transmitter<TodoIn>, rx: Receiver<TodoOut>) -> GizmoBuilder {
    li()
      .rx_class("", rx.branch_filter_map(|msg| msg.as_list_class()))
      .rx_style("display", "block", rx.branch_filter_map(|msg| {
        match msg {
          TodoOut::SetVisible(visible) => {
            Some(
              if *visible {
                "block"
              } else {
                "none"
              }.to_string()
            )
          }
          _ => { None }
        }
      }))
      .with(
        div()
          .class("view")
          .with(
            input()
              .tx_post_build(
                tx.contra_map(|el:&HtmlElement| {
                  TodoIn::CompletionToggleInput(el.clone())
                })
              )
              .class("toggle")
              .attribute("type", "checkbox")
              .style("cursor", "pointer")
              .tx_on("click", tx.contra_map(|_:&Event| TodoIn::ToggleCompletion))
          )
          .with(
            label()
              .rx_text(&self.name, rx.branch_filter_map(|msg| {
                match msg {
                  TodoOut::SetName(name) => { Some(name.clone()) }
                  _ => { None }
                }
              }))
              .tx_on("dblclick", tx.contra_map(|_:&Event| TodoIn::StartEditing))
          )
          .with(
            button()
              .class("destroy")
              .style("cursor", "pointer")
              .tx_on("click", tx.contra_map(|_:&Event| TodoIn::Remove))
          )
      )
      .with(
        input()
          .tx_post_build(
            tx.contra_map(|el:&HtmlElement| TodoIn::EditInput(el.clone()))
          )
          .class("edit")
          .value(&self.name, )
          .tx_on("blur", tx.contra_map(|_:&Event| TodoIn::StopEditing(None)))
          .tx_on("keyup", tx.contra_map(|ev:&Event| TodoIn::StopEditing(Some(ev.clone()))))
      )
  }
}

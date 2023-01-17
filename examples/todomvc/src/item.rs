//! Provides a todo line-item that can be edited by double clicking,
//! marked as complete or removed.
use mogwai_dom::{
    core::{model::Model, stream, either::Either},
    prelude::*,
};
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, KeyboardEvent};

/// Used to set the todo item's `<li>` class
pub enum ItemClass {
    None,
    Editing,
    Completed,
}

impl ItemClass {
    fn is_done(is_done: bool) -> Self {
        if is_done {
            ItemClass::Completed
        } else {
            ItemClass::None
        }
    }

    // see as_list_class
    fn to_string(self) -> String {
        match self {
            ItemClass::None => "",
            ItemClass::Editing => "editing",
            ItemClass::Completed => "completed",
        }
        .to_string()
    }
}

/// Determines the source of a "stop editing" event.
#[derive(Clone, Debug)]
enum StopEditingEvent {
    Enter,
    Escape,
    Blur,
}

/// Messages that come out of the todo item and out to the list.
#[derive(Clone)]
pub enum TodoItemMsg {
    Completion,
    Remove(usize),
}

/// Messages that come from the list into the todo item via
/// pub async functions exposed on [`TodoItem`].
#[derive(Clone, PartialEq)]
enum ListItemMsg {
    SetComplete(bool),
    SetVisible(bool),
}

#[derive(Clone)]
pub struct TodoItem {
    pub id: usize,
    pub complete: Model<bool>,
    pub name: Model<String>,
    output_to_list: Output<TodoItemMsg>,
    input_to_item: FanInput<ListItemMsg>,
}

impl TodoItem {
    pub fn new(
        id: usize,
        name: impl Into<String>,
        complete: bool,
        output_to_list: Output<TodoItemMsg>,
    ) -> Self {
        let input_to_item = FanInput::default();
        TodoItem {
            name: Model::new(name.into()),
            complete: Model::new(complete),
            id,
            output_to_list,
            input_to_item,
        }
    }

    pub async fn set_complete(&self, complete: bool) {
        self.input_to_item
            .set(ListItemMsg::SetComplete(complete))
            .await
            .expect("could not set complete");
    }

    pub async fn set_visible(&self, visible: bool) {
        self.input_to_item
            .set(ListItemMsg::SetVisible(visible))
            .await
            .expect("could not set visible")
    }

    fn stream_of_is_visible_display(&self) -> impl Stream<Item = String> {
        stream::iter(std::iter::once("block".to_string())).chain(
            self.input_to_item.stream().filter_map(|msg| match msg {
                ListItemMsg::SetVisible(is_visible) => {
                    Some(if is_visible { "block" } else { "none" }.to_string())
                }
                _ => None,
            }),
        )
    }

    async fn task_start_editing(
        self,
        captured_edit_input: Captured<JsDom>,
        input_item_class: Input<ItemClass>,
        output_label_double_clicked: Output<()>,
    ) {
        let edit_input: JsDom = captured_edit_input.get().await;
        let starting_name: String = self.name.read().await.clone();
        edit_input.visit_as(|el: &HtmlInputElement| el.set_value(&starting_name));
        while let Some(()) = output_label_double_clicked.get().await {
            // set the input to "editing"
            input_item_class
                .set(ItemClass::Editing)
                .await
                .expect("can't set editing class");
            // give a moment for the class to update and make the input editable
            mogwai_dom::core::time::wait_millis(10).await;
            // focus the input
            edit_input.visit_as(|el: &HtmlInputElement| el.focus().expect("can't focus"));
        }
    }

    async fn task_stop_editing(
        self,
        captured_edit_input: Captured<JsDom>,
        input_item_class: Input<ItemClass>,
        output_edit_onkeyup: Output<JsDomEvent>,
        output_edit_onblur: Output<()>,
    ) {
        let edit_input = captured_edit_input.get().await;
        let on_keyup = output_edit_onkeyup.get_stream().map(Either::Left);
        let on_blur = output_edit_onblur.get_stream().map(Either::Right);
        let mut events = on_keyup.boxed().or(on_blur.boxed());
        while let Some(e) = events.next().await {
            log::info!("stop editing event");
            let may_edit_event = match e {
                // keyup
                Either::Left(ev) => {
                    log::info!("  keyup");
                    // Get the browser event or filter on non-wasm targets.
                    let ev = ev.browser_event().expect("can't get keyup event");
                    // This came from a key event
                    let kev = ev.unchecked_ref::<KeyboardEvent>();
                    let key = kev.key();
                    if key == "Enter" {
                        Some(StopEditingEvent::Enter)
                    } else if key == "Escape" {
                        Some(StopEditingEvent::Escape)
                    } else {
                        None
                    }
                }
                // blur
                Either::Right(()) => {
                    log::info!("  blur");
                    Some(StopEditingEvent::Blur)
                }
            };

            if let Some(ev) = may_edit_event {
                match ev {
                    StopEditingEvent::Enter | StopEditingEvent::Blur => {
                        let input_name = edit_input
                            .visit_as(|i: &HtmlInputElement| crate::utils::input_value(i).unwrap());
                        if let Some(s) = input_name {
                            self.name
                                .visit_mut(|name| {
                                    *name = s;
                                })
                                .await;
                        }
                    }
                    StopEditingEvent::Escape => {
                        let name = self.name.read().await.clone();
                        edit_input
                            .visit_as(|i: &HtmlInputElement| i.set_value(&name))
                            .unwrap();
                    }
                }
                input_item_class
                    .set(ItemClass::None)
                    .await
                    .expect("can't set editing class");
            }
        }
    }

    async fn task_toggle_complete(
        self,
        captured_complete_toggle: Captured<JsDom>,
        output_complete_toggle_clicked: Output<()>,
    ) {
        let toggle_input_element = captured_complete_toggle.get().await;
        let done = *self.complete.read().await;
        toggle_input_element.visit_as(|el: &HtmlInputElement| el.set_checked(done));

        let mut events = self.input_to_item.stream().map(Either::Left).boxed().or(
            output_complete_toggle_clicked
                .get_stream()
                .map(Either::Right)
                .boxed(),
        );
        while let Some(ev) = events.next().await {
            match ev {
                Either::Left(ListItemMsg::SetComplete(done)) => {
                    toggle_input_element
                        .visit_as(|el: &HtmlInputElement| el.set_checked(done))
                        .expect("could not set checked");
                    self.complete.visit_mut(|c| *c = done).await;
                }
                Either::Left(_) => {}
                Either::Right(()) => {
                    let done = toggle_input_element
                        .visit_as(|el: &HtmlInputElement| el.checked())
                        .unwrap_or_default();
                    let _ = self.complete.visit_mut(|d| *d = done).await;
                    let _ = self.output_to_list.send(TodoItemMsg::Completion).await;
                }
            }
        }
    }

    async fn task_remove_item(self, output_remove_button_clicked: Output<()>) {
        while let Some(()) = output_remove_button_clicked.get().await {
            self.output_to_list
                .send(TodoItemMsg::Remove(self.id))
                .await
                .expect("could not send removal");
        }
    }

    pub fn viewbuilder(self) -> ViewBuilder {
        let captured_complete_toggle_dom = Captured::<JsDom>::default();
        let captured_edit_input = Captured::<JsDom>::default();

        let mut input_item_class = Input::<ItemClass>::default();

        let output_complete_toggle_clicked = Output::<()>::default();
        let output_remove_button_clicked = Output::<()>::default();
        let output_label_double_clicked = Output::<()>::default();
        let output_edit_onblur = Output::<()>::default();
        let output_edit_onkeyup = Output::<JsDomEvent>::default();

        let builder = rsx! {
            li(
                class = (
                    ItemClass::is_done(self.complete.current().expect("could not read complete")).to_string(),
                    input_item_class.stream().unwrap()
                        .map(ItemClass::to_string).boxed()
                        .or(
                            self.complete.stream().map(|done| ItemClass::is_done(done).to_string()).boxed()
                        )
                ),
                style:display = self.stream_of_is_visible_display()
            ) {
                div(class="view") {
                    input(
                        class = "toggle",
                        type_ = "checkbox",
                        style:cursor = "pointer",
                        capture:view = captured_complete_toggle_dom.sink(),
                        on:click = output_complete_toggle_clicked.sink().contra_map(|_:JsDomEvent| ())
                    ){}

                    label(on:dblclick = output_label_double_clicked.sink().contra_map(|_:JsDomEvent| ())) {
                        {(
                            self.name.current().expect("current name"),
                            self.name.stream()
                        )}
                    }

                    button(
                        class = "destroy",
                        style = "cursor: pointer;",
                        on:click = output_remove_button_clicked.sink().contra_map(|_:JsDomEvent| ()),
                    ){}
                }

                input(
                    class = "edit",
                    capture:view = captured_edit_input.sink(),
                    on:blur = output_edit_onblur.sink().contra_map(|_:JsDomEvent| ()),
                    on:keyup = output_edit_onkeyup.sink()
                ){}
            }
        };
        builder
            .with_task(self.clone().task_start_editing(
                captured_edit_input.clone(),
                input_item_class.clone(),
                output_label_double_clicked,
            ))
            .with_task(self.clone().task_stop_editing(
                captured_edit_input,
                input_item_class,
                output_edit_onkeyup,
                output_edit_onblur,
            ))
            .with_task(
                self.clone().task_toggle_complete(
                    captured_complete_toggle_dom,
                    output_complete_toggle_clicked,
                ),
            )
            .with_task(self.task_remove_item(output_remove_button_clicked))
    }
}

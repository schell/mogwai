use mogwai::prelude::*;
use web_sys::{HtmlInputElement, KeyboardEvent};
use wasm_bindgen::JsCast;

use super::{utils, FilterShow};

//#[derive(Default)]
//pub struct TodoItem {
//    // Index used to identify the item
//    pub index: usize,
//
//    /// The completion status has been updated
//    pub completion_changed: Output<bool>,
//    /// The remove button was clicked
//    pub clicked_remove: Output<()>,
//}
//
//impl Relay<Dom> for TodoItem {
//    type Error = ();
//
//    fn view(&mut self) -> ViewBuilder<Dom> {
//        builder! {
//            <li class=rx.clone().filter_map(|msg| async move {msg.as_list_class()})
//                style:display=(
//                    "block",
//                    rx.clone().filter_map(|msg| async move {
//                        match msg {
//                            ItemView::SetVisible(visible) => {
//                                Some(if visible { "block" } else { "none" }.to_string())
//                            }
//                            _ => None,
//                        }
//                    })
//                )>
//                <div class="view">
//                    <input class="toggle" type="checkbox" style:cursor="pointer"
//                    capture:view= send_completion_toggle_input
//                    on:click=tx.clone().contra_map(|_| ItemLogic::ToggleCompletion)
//                    />
//                    <label on:dblclick=tx.clone().contra_map(|_| ItemLogic::StartEditing)>
//                        {(
//                            name,
//                            rx.filter_map(|msg| async move {
//                                match msg {
//                                    ItemView::SetName(name) => Some(name.clone()),
//                                    _ => None,
//                                }
//                            })
//                        )}
//                    </label>
//                    <button
//                        class="destroy"
//                        style="cursor: pointer;"
//                        on:click=tx.clone().contra_map(|_| ItemLogic::Remove) />
//                </div>
//                <input
//                class="edit"
//                capture:view=send_edit_input
//                on:blur=tx.clone().contra_map(|_| ItemLogic::StopEditing(EditEvent::Blur))
//                on:keyup=tx.clone().contra_filter_map(|ev: DomEvent| {
//                    // Get the browser event or filter on non-wasm targets.
//                    let ev = ev.browser_event()?;
//                    // This came from a key event
//                    let kev = ev.unchecked_ref::<KeyboardEvent>();
//                    let key = kev.key();
//                    let cmd = if key == "Enter" {
//                        Some(EditEvent::Enter)
//                    } else if key == "Escape" {
//                        Some(EditEvent::Escape)
//                    } else {
//                        None //EditEvent::OtherKeydown
//                    };
//                    cmd.map(|cmd| ItemLogic::StopEditing(cmd))
//                })
//                />
//            </li>
//        }
//    }
//}

#[derive(Clone)]
// ANCHOR: todo_struct
pub struct Todo {
    pub index: usize,
    tx_logic: broadcast::Sender<ItemLogic>,
    rx_changed_completion: broadcast::Receiver<bool>,
    rx_removed: broadcast::Receiver<()>,
}
// ANCHOR_END: todo_struct

impl Drop for Todo {
    fn drop(&mut self) {
        let _ = self.tx_logic.close();
    }
}

impl Todo {
    /// Create a new todo item by returning a Todo and its ViewBuilder.
    /// from the item.
    pub fn new(index: usize, name: impl Into<String>) -> (Todo, ViewBuilder<Dom>) {
        let name = name.into();

        let (send_completion_toggle_input, recv_completion_toggle_input) = mpsc::bounded(1);
        let (send_edit_input, recv_edit_input) = mpsc::bounded(1);
        let (tx_logic, rx_logic) = broadcast::bounded(1);
        let (tx_view, rx_view) = broadcast::bounded(1);
        let (mut tx_changed_completion, rx_changed_completion) = broadcast::bounded::<bool>(1);
        tx_changed_completion.inner.set_overflow(true);
        let (mut tx_removed, rx_removed) = broadcast::bounded::<()>(1);
        tx_removed.inner.set_overflow(true);

        let view_builder = view(
            &name,
            send_completion_toggle_input,
            send_edit_input,
            tx_logic.clone(),
            rx_view.clone(),
        );

        spawn(logic(
            name.to_string(),
            recv_completion_toggle_input,
            recv_edit_input,
            rx_logic,
            tx_view,
            tx_changed_completion,
            tx_removed,
        ));

        (
            Todo {
                index,
                tx_logic,
                rx_changed_completion,
                rx_removed
            },
            view_builder,
        )
    }

    // ANCHOR: use_tx_logic
    pub async fn as_item(&self) -> crate::store::Item {
        let (tx, mut rx) = futures::channel::mpsc::channel(1);
        self.tx_logic
            .broadcast(ItemLogic::QueryItem(tx))
            .await
            .unwrap();
        rx.next().await.unwrap()
    }

    pub async fn filter(&self, fs: FilterShow) {
        let (tx, mut rx) = futures::channel::mpsc::channel(1);
        self.tx_logic
            .broadcast(ItemLogic::SetFilterShow(fs, tx))
            .await
            .unwrap();
        rx.next().await.unwrap();
    }

    /// Return whether this todo has been marked done.
    pub async fn is_done(&self) -> bool {
        let (tx, mut rx) = futures::channel::mpsc::channel::<bool>(1);
        self.tx_logic
            .broadcast(ItemLogic::QueryIsDone(tx))
            .await
            .unwrap();
        rx.next().await.unwrap()
    }

    pub async fn set_complete(&self, complete: bool) {
        self.tx_logic
            .broadcast(ItemLogic::SetCompletion(complete))
            .await
            .unwrap();
    }
    // ANCHOR_END: use_tx_logic

    pub fn has_changed_completion(&self) -> impl Stream<Item = bool> {
        self.rx_changed_completion.clone()
    }

    pub fn was_removed(&self) -> impl Stream<Item = ()> {
        self.rx_removed.clone()
    }
}

#[derive(Clone, Debug)]
pub enum EditEvent {
    Enter,
    Escape,
    Blur,
}

/// Messages sent from the view to the logic loop.
#[derive(Clone, Debug)]
enum ItemLogic {
    ToggleCompletion,
    SetCompletion(bool),
    QueryIsDone(futures::channel::mpsc::Sender<bool>),
    QueryItem(futures::channel::mpsc::Sender<crate::store::Item>),
    StartEditing,
    StopEditing(EditEvent),
    SetFilterShow(FilterShow, futures::channel::mpsc::Sender<()>),
    Remove,
}

/// Messages sent from the logic loop to the view.
#[derive(Clone)]
enum ItemView {
    UpdateEditComplete(bool, bool),
    SetName(String),
    SetVisible(bool),
}

impl ItemView {
    fn as_list_class(&self) -> Option<String> {
        match self {
            ItemView::UpdateEditComplete(editing, completed) => Some(
                if *editing {
                    "editing"
                } else if *completed {
                    "completed"
                } else {
                    ""
                }
                .to_string(),
            ),
            _ => None,
        }
    }
}

async fn logic(
    name: String,
    mut recv_toggle_input: impl Stream<Item = Dom> + Unpin,
    mut recv_edit_input: impl Stream<Item = Dom> + Unpin,
    mut rx_logic: broadcast::Receiver<ItemLogic>,
    tx_view: broadcast::Sender<ItemView>,
    tx_changed_completion: broadcast::Sender<bool>,
    tx_removed: broadcast::Sender<()>,
) {
    let mut name = name;
    let mut is_editing = false;

    let toggle_input = recv_toggle_input.next().await.unwrap();
    let edit_input = recv_edit_input.next().await.unwrap();
    edit_input.visit_as(|input: &HtmlInputElement| input.set_value(&name), |_| ());

    while let Some(msg) = rx_logic.next().await {
        // ANCHOR: facade_logic_loop
        match msg {
            ItemLogic::QueryIsDone(mut tx) => {
                let is_done = toggle_input
                    .visit_as(|i: &HtmlInputElement| i.checked(), |_| false)
                    .unwrap_or(false);
                tx.send(is_done).await.unwrap();
            }
            ItemLogic::QueryItem(mut tx) => {
                let is_done = toggle_input
                    .visit_as(|i: &HtmlInputElement| i.checked(), |_| false)
                    .unwrap_or(false);
                tx.send(crate::store::Item {
                    title: name.clone(),
                    completed: is_done,
                })
                .await
                .unwrap();
            }
        // ANCHOR_END: facade_logic_loop
            ItemLogic::SetFilterShow(show, mut tx) => {
                let is_done = toggle_input
                    .visit_as(|i: &HtmlInputElement| i.checked(), |_| false)
                    .unwrap_or(false);
                let is_visible = show == FilterShow::All
                    || (show == FilterShow::Completed && is_done)
                    || (show == FilterShow::Active && !is_done);
                tx_view
                    .broadcast(ItemView::SetVisible(is_visible))
                    .await
                    .unwrap();
                tx.send(()).await.unwrap();
            }
            ItemLogic::ToggleCompletion => {
                let is_done = toggle_input
                    .visit_as(|i: &HtmlInputElement| i.checked(), |_| false)
                    .unwrap_or(false);
                tx_view
                    .broadcast(ItemView::UpdateEditComplete(is_editing, is_done))
                    .await
                    .unwrap();
                tx_changed_completion
                    .broadcast(is_done)
                    .await
                    .unwrap();
            }
            ItemLogic::SetCompletion(completed) => {
                toggle_input.visit_as(|i: &HtmlInputElement| i.set_checked(completed), |_| ());
                tx_view
                    .broadcast(ItemView::UpdateEditComplete(is_editing, completed))
                    .await
                    .unwrap();
            }
            ItemLogic::StartEditing => {
                is_editing = true;
                let _ = mogwai::core::time::wait_secs(1.0).await;
                edit_input.visit_as(
                    |i: &HtmlInputElement| i.focus().expect("can't focus"),
                    |_| (),
                );
                let is_done = toggle_input
                    .visit_as(|i: &HtmlInputElement| i.checked(), |_| false)
                    .unwrap_or(false);
                tx_view
                    .broadcast(ItemView::UpdateEditComplete(is_editing, is_done))
                    .await
                    .unwrap();
            }
            ItemLogic::StopEditing(ev) => {
                is_editing = false;

                match ev {
                    EditEvent::Enter | EditEvent::Blur => edit_input
                        .visit_as(
                            |i: &HtmlInputElement| {
                                if let Some(s) = utils::input_value(i) {
                                    name = s;
                                }
                            },
                            |_| (),
                        )
                        .unwrap(),
                    EditEvent::Escape => edit_input
                        .visit_as(|i: &HtmlInputElement| i.set_value(&name), |_| ())
                        .unwrap(),
                }

                let is_done = toggle_input
                    .visit_as(|i: &HtmlInputElement| i.checked(), |_| false)
                    .unwrap_or(false);
                tx_view
                    .broadcast(ItemView::SetName(name.to_string()))
                    .await
                    .unwrap();
                tx_view
                    .broadcast(ItemView::UpdateEditComplete(is_editing, is_done))
                    .await
                    .unwrap();
                tx_changed_completion
                    .broadcast(is_done)
                    .await
                    .unwrap();
            }
            ItemLogic::Remove => {
                // The todo sends a message to the parent App to be removed.
                tx_removed.broadcast(()).await.unwrap();
            }
        }
    }
}

fn view(
    name: &str,
    send_completion_toggle_input: mpsc::Sender<Dom>,
    send_edit_input: mpsc::Sender<Dom>,
    tx: broadcast::Sender<ItemLogic>,
    rx: broadcast::Receiver<ItemView>,
) -> ViewBuilder<Dom> {
    builder! {
        <li class=rx.clone().filter_map(|msg| async move {msg.as_list_class()})
            style:display=(
                "block",
                rx.clone().filter_map(|msg| async move {
                    match msg {
                        ItemView::SetVisible(visible) => {
                            Some(if visible { "block" } else { "none" }.to_string())
                        }
                        _ => None,
                    }
                })
            )>
            <div class="view">
                <input class="toggle" type="checkbox" style:cursor="pointer"
                 capture:view= send_completion_toggle_input
                 on:click=tx.clone().contra_map(|_| ItemLogic::ToggleCompletion)
                />
                <label on:dblclick=tx.clone().contra_map(|_| ItemLogic::StartEditing)>
                    {(
                        name,
                        rx.filter_map(|msg| async move {
                            match msg {
                                ItemView::SetName(name) => Some(name.clone()),
                                _ => None,
                            }
                        })
                    )}
                </label>
                <button
                    class="destroy"
                    style="cursor: pointer;"
                    on:click=tx.clone().contra_map(|_| ItemLogic::Remove) />
            </div>
            <input
             class="edit"
             capture:view=send_edit_input
             on:blur=tx.clone().contra_map(|_| ItemLogic::StopEditing(EditEvent::Blur))
             on:keyup=tx.clone().contra_filter_map(|ev: DomEvent| {
                 // Get the browser event or filter on non-wasm targets.
                 let ev = ev.browser_event()?;
                 // This came from a key event
                 let kev = ev.unchecked_ref::<KeyboardEvent>();
                 let key = kev.key();
                 let cmd = if key == "Enter" {
                     Some(EditEvent::Enter)
                 } else if key == "Escape" {
                     Some(EditEvent::Escape)
                 } else {
                     None //EditEvent::OtherKeydown
                 };
                 cmd.map(|cmd| ItemLogic::StopEditing(cmd))
             })
             />
        </li>
    }
}

use mogwai::prelude::*;
use web_sys::{HtmlInputElement, KeyboardEvent};

use super::{utils, FilterShow};

#[derive(Clone)]
pub struct Todo {
    pub index: usize,
    tx_logic: broadcast::Sender<ItemLogic>,
    rx_out: broadcast::Receiver<ItemOut>,
}

impl Todo {
    /// Create a new todo item by returning a Todo and its ViewBuilder.
    /// from the item.
    pub fn new(index: usize, name: impl Into<String>) -> (Todo, ViewBuilder<Dom>) {
        let name = name.into();

        let (send_completion_toggle_input, recv_completion_toggle_input) = mpmc::bounded(1);
        let (send_edit_input, recv_edit_input) = mpmc::bounded(1);
        let (tx_logic, rx_logic) = broadcast::bounded(1);
        let (tx_view, rx_view) = broadcast::bounded(1);
        let (tx_out, rx_out) = broadcast::bounded::<ItemOut>(1);

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
            tx_out,
        ));

        (
            Todo {
                index,
                tx_logic,
                rx_out,
            },
            view_builder,
        )
    }

    pub async fn as_item(&self) -> crate::store::Item {
        let (tx, rx) = mpmc::bounded(1);
        self.tx_logic
            .broadcast(ItemLogic::QueryItem(tx))
            .await
            .unwrap();
        rx.recv().await.unwrap()
    }

    pub async fn filter(&self, fs: FilterShow) {
        let (tx, rx) = mpmc::bounded(1);
        self.tx_logic
            .broadcast(ItemLogic::SetFilterShow(fs, tx))
            .await
            .unwrap();
        rx.recv().await.unwrap();
    }

    /// Return whether this todo has been marked done.
    pub async fn is_done(&self) -> bool {
        let (tx, rx) = mpmc::bounded::<bool>(1);
        self.tx_logic
            .broadcast(ItemLogic::QueryIsDone(tx))
            .await
            .unwrap();
        rx.recv().await.unwrap()
    }

    pub async fn set_complete(&self, complete: bool) {
        self.tx_logic
            .broadcast(ItemLogic::SetCompletion(complete))
            .await
            .unwrap();
    }

    pub fn has_changed_completion(&self) -> impl Stream<Item = bool> {
        let rx = self.rx_out.clone();
        rx.filter_map(|msg| async move {
            match msg {
                ItemOut::Remove => None,
                ItemOut::IsComplete(complete) => Some(complete),
            }
        })
    }

    pub fn was_removed(&self) -> impl Stream<Item = ()> {
        let rx = self.rx_out.clone();
        rx.filter_map(|msg| async move {
            match msg {
                ItemOut::Remove => Some(()),
                ItemOut::IsComplete(_) => None,
            }
        })
    }
}

#[derive(Clone)]
pub enum EditEvent {
    Enter,
    Escape,
    OtherKeydown,
    Blur,
}

/// Messages sent from the view to the logic loop.
#[derive(Clone)]
enum ItemLogic {
    ToggleCompletion,
    SetCompletion(bool),
    QueryIsDone(mpmc::Sender<bool>),
    QueryItem(mpmc::Sender<crate::store::Item>),
    StartEditing,
    StopEditing(EditEvent),
    SetFilterShow(FilterShow, mpmc::Sender<()>),
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

#[derive(Clone)]
enum ItemOut {
    Remove,
    IsComplete(bool),
}

async fn logic(
    name: String,
    mut recv_toggle_input: impl Stream<Item = Dom> + Unpin,
    mut recv_edit_input: impl Stream<Item = Dom> + Unpin,
    mut rx_logic: impl Stream<Item = ItemLogic> + Unpin,
    tx_view: broadcast::Sender<ItemView>,
    tx_out: broadcast::Sender<ItemOut>,
) {
    let mut name = name;
    let mut is_editing = false;
    let mut is_done = false;

    let toggle_input = recv_toggle_input.next().await.unwrap();
    let edit_input = recv_edit_input.next().await.unwrap();
    edit_input.visit_as::<HtmlInputElement, _, _>(|input| input.set_value(&name), |_| ());

    loop {
        match rx_logic.next().await {
            Some(ItemLogic::QueryIsDone(tx)) => {
                tx.send(is_done).await.unwrap();
            }
            Some(ItemLogic::QueryItem(tx)) => {
                tx.send(crate::store::Item {
                    title: name.clone(),
                    completed: is_done,
                })
                .await
                .unwrap();
            }
            Some(ItemLogic::SetFilterShow(show, tx)) => {
                let is_visible = show == FilterShow::All
                    || (show == FilterShow::Completed && is_done)
                    || (show == FilterShow::Active && !is_done);
                tx_view
                    .broadcast(ItemView::SetVisible(is_visible))
                    .await
                    .unwrap();
                tx.send(()).await.unwrap();
            }
            Some(ItemLogic::ToggleCompletion) => {
                is_done = !is_done;
                tx_view
                    .broadcast(ItemView::UpdateEditComplete(is_editing, is_done))
                    .await
                    .unwrap();
                tx_out
                    .broadcast(ItemOut::IsComplete(is_done))
                    .await
                    .unwrap();
            }
            Some(ItemLogic::SetCompletion(completed)) => {
                is_done = completed;
                toggle_input
                    .visit_as::<HtmlInputElement, _, _>(|i| i.set_checked(completed), |_| ());
                tx_view
                    .broadcast(ItemView::UpdateEditComplete(is_editing, is_done))
                    .await
                    .unwrap();
            }
            Some(ItemLogic::StartEditing) => {
                is_editing = true;
                let _ = mogwai::time::wait_approx(1.0).await;
                edit_input.visit_as(
                    |i: &HtmlInputElement| i.focus().expect("can't focus"),
                    |_| (),
                );
                tx_view
                    .broadcast(ItemView::UpdateEditComplete(is_editing, is_done))
                    .await
                    .unwrap();
            }
            Some(ItemLogic::StopEditing(ev)) => {
                is_editing = false;

                match ev {
                    EditEvent::Enter | EditEvent::Blur => edit_input.visit_as(
                        |i: &HtmlInputElement| {
                            if let Some(s) = utils::input_value(i) {
                                name = s;
                            }
                        },
                        |_| (),
                    ),
                    EditEvent::Escape => {
                        edit_input.visit_as(|i: &HtmlInputElement| i.set_value(&name), |_| ());
                    }
                    EditEvent::OtherKeydown => {}
                }

                tx_view
                    .broadcast(ItemView::SetName(name.to_string()))
                    .await
                    .unwrap();
                tx_view
                    .broadcast(ItemView::UpdateEditComplete(is_editing, is_done))
                    .await
                    .unwrap();
                tx_out
                    .broadcast(ItemOut::IsComplete(is_done))
                    .await
                    .unwrap_or_default();
            }
            Some(ItemLogic::Remove) => {
                // The todo sends a message to the parent App to be removed.
                tx_out.broadcast(ItemOut::Remove).await.unwrap();
            }
            None => break,
        }
    }
}

fn view(
    name: &str,
    send_completion_toggle_input: mpmc::Sender<Dom>,
    send_edit_input: mpmc::Sender<Dom>,
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
                 post:build=move |dom:&mut Dom| {
                     send_completion_toggle_input.try_send(dom.clone()).unwrap();
                 }
                 on:click=tx.sink().with(|_| async{Ok(ItemLogic::ToggleCompletion)})
                />
                <label on:dblclick=tx.sink().with(|_| async{Ok(ItemLogic::StartEditing)})>
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
                    on:click=tx.sink().with(|_| async{Ok(ItemLogic::Remove)}) />
            </div>
            <input
             class="edit"
             post:build=move |dom: &mut Dom| {
                 log::info!("sending edit input");
                 send_edit_input.try_send(dom.clone()).unwrap();
             }
             on:blur=tx.sink().with(|_| async{Ok(ItemLogic::StopEditing(EditEvent::Blur))})
             on:keyup=tx.sink().with(|ev: Event| {
                 // This came from a key event
                 let kev = ev.unchecked_ref::<KeyboardEvent>();
                 let key = kev.key();
                 let cmd = if key == "Enter" {
                     EditEvent::Enter
                 } else if key == "Escape" {
                     EditEvent::Escape
                 } else {
                     EditEvent::OtherKeydown
                 };
                 async{Ok(ItemLogic::StopEditing(cmd))}
             })
             />
        </li>
    }
}

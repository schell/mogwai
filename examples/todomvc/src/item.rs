//! Provides a todo line-item that can be edited by double clicking,
//! marked as complete or removed.
use futures::FutureExt;
use mogwai::web::prelude::*;
use web_sys::wasm_bindgen::{JsCast, UnwrapThrowExt};

use crate::app::FilterShow;

#[derive(Default)]
pub struct ItemState {
    pub is_editing: bool,
    pub is_completed: bool,
    pub is_visible: bool,
    pub name: String,
}

impl ItemState {
    // see as_list_class
    fn as_list_class(&self) -> &'static str {
        if self.is_editing {
            "editing"
        } else if self.is_completed {
            "completed"
        } else {
            ""
        }
    }
}

#[derive(ViewChild)]
pub struct TodoItem<V: View> {
    id: usize,
    #[child]
    wrapper: V::Element,
    completed_input: V::Element,

    input_edit: V::Element,

    on_click_completed_toggle: V::EventListener,
    on_dblclick_name: V::EventListener,
    on_click_destroy: V::EventListener,
    on_blur_edit: V::EventListener,
    on_keyup_edit: V::EventListener,

    state: Proxy<ItemState>,
}

impl<V: View> TodoItem<V> {
    pub fn new(id: usize, name: impl AsRef<str>, complete: bool) -> Self {
        let mut state = Proxy::<ItemState>::new(ItemState {
            name: name.as_ref().into(),
            is_editing: false,
            is_completed: complete,
            is_visible: true,
        });

        rsx! {
            let wrapper = li(
                class = state(s => s.as_list_class()),
                style:display = state(s => if s.is_visible { "block" } else { "none" })
            ) {
                div(class="view") {
                    let completed_input = input(
                        class = "toggle",
                        type_ = "checkbox",
                        style:cursor = "pointer",
                        on:click = on_click_completed_toggle
                    ){}

                    label(on:dblclick = on_dblclick_name) {
                        {state(s => &s.name)}
                    }

                    button(
                        class = "destroy",
                        style = "cursor: pointer;",
                        on:click = on_click_destroy,
                    ){}
                }
                let input_edit = input(
                    class = "edit",
                    on:blur = on_blur_edit,
                    on:keyup = on_keyup_edit
                ) {}
            }
        }

        Self {
            id,
            wrapper,
            completed_input,
            input_edit,
            on_click_completed_toggle,
            on_dblclick_name,
            on_click_destroy,
            on_blur_edit,
            on_keyup_edit,
            state,
        }
    }

    pub fn filter(&mut self, filter: FilterShow) {
        match filter {
            FilterShow::All => self.state.modify(|s| s.is_visible = true),
            FilterShow::Completed => self.state.modify(|s| s.is_visible = s.is_completed),
            FilterShow::Active => self.state.modify(|s| s.is_visible = !s.is_completed),
        }
    }

    pub fn set_completed(&mut self, complete: bool) {
        self.state.modify(|s| s.is_completed = complete);
        self.completed_input.when_element::<Web, _>(|el| {
            let input = el.dyn_ref::<web_sys::HtmlInputElement>().unwrap_throw();
            input.set_checked(complete);
        });
    }

    pub fn get_completed(&self) -> bool {
        self.state.is_completed
    }

    pub fn get_name(&self) -> String {
        self.state.name.clone()
    }

    /// Run the item until an event occurs, possibly returning the id of the item
    /// if it's set for destruction.
    pub async fn run_step(&mut self) -> Option<usize> {
        enum Step<V: View> {
            StartEditing,
            StopEditingBlur(V::Event),
            StopEditingKeyup(V::Event),
            Destroy,
            None,
        }

        let mut step = Step::<V>::None;
        if self.state.is_editing {
            // Editing mode, in which we wait for editing to end
            futures::select! {
                ev = self.on_blur_edit.next().fuse() => {
                    step = Step::StopEditingBlur(ev);

                }
                ev = self.on_keyup_edit.next().fuse() => {
                    step = Step::StopEditingKeyup(ev);
                }
            }
        } else {
            // Default mode
            futures::select! {
                _ = self.on_click_completed_toggle.next().fuse() => {
                    self.state.modify(|s| s.is_completed = !s.is_completed);
                }
                _ = self.on_dblclick_name.next().fuse() => {
                    step = Step::StartEditing;
                }
                _ = self.on_click_destroy.next().fuse() => {
                    step = Step::Destroy;
                }
            }
        }

        let mut destroy = None;
        match step {
            Step::StartEditing => {
                log::info!("started editing");
                self.state.modify(|s| s.is_editing = true);
                self.input_edit
                    .dyn_el::<web_sys::HtmlInputElement, _>(|input| {
                        input.focus().unwrap_throw();
                    });
            }
            Step::StopEditingBlur(ev) => {
                log::info!("stop editing blur");
                let name = ev
                    .dyn_ev(crate::utils::event_input_value)
                    .flatten()
                    .unwrap_or_else(|| self.state.name.clone());
                self.state.modify(|s| {
                    s.is_editing = false;
                    s.name = name;
                });
            }
            Step::StopEditingKeyup(ev) => {
                ev.dyn_ev::<web_sys::KeyboardEvent, _>(|ev| {
                    let key = ev.key();
                    match key.as_str() {
                        "Enter" => {
                            let name = crate::utils::event_input_value(ev)
                                .unwrap_or_else(|| self.state.name.clone());
                            self.state.modify(|s| {
                                s.is_editing = false;
                                s.name = name;
                            });
                        }
                        "Escape" => {
                            self.input_edit
                                .dyn_el::<web_sys::HtmlInputElement, _>(|input| {
                                    input.set_value("")
                                });
                            self.state.modify(|s| s.is_editing = false);
                        }
                        _ => {}
                    }
                });
            }
            Step::Destroy => {
                destroy = Some(self.id);
            }
            Step::None => {}
        }
        destroy
    }
}

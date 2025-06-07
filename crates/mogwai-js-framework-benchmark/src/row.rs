//! The row.
use std::ops::Deref;

use mogwai::web::{prelude::*, Global};
use wasm_bindgen::{JsCast, UnwrapThrowExt};

pub struct RowModel {
    pub id: usize,
    pub label: Str,
}

pub struct RowViewTemplate<V: View> {
    wrapper: V::Element,
}

impl<V: View> Default for RowViewTemplate<V> {
    fn default() -> Self {
        rsx! {
            let wrapper = tr(class = "has_key_attr") {
                td(class="col-md-1 has_key_text"){ "" }
                td(class="col-md-4"){
                    a(class = "lbl has_key_attr has_model_text", key = "0") { "" }
                }
                td(class="col-md-1"){
                    a(class="remove" ) {
                        span(
                            class="remove glyphicon glyphicon-remove has_key_attr",
                            key = "0",
                            aria_hidden="true"
                        ) {}
                    }
                }
                td(class="col-md-6"){ }
            }
        }

        Self { wrapper }
    }
}

static ROW_VIEW_TEMPLATE: Global<web_sys::Element> =
    Global::new(|| RowViewTemplate::<Web>::default().wrapper);

#[derive(Clone)]
pub struct RowView {
    wrapper: web_sys::Element,
    key_attrs: web_sys::NodeList,
    key_text: web_sys::Text,
    model_text: web_sys::Text,
}

impl Default for RowView {
    fn default() -> Self {
        let template: &web_sys::Element = ROW_VIEW_TEMPLATE.deref();
        let wrapper = template.clone_node_with_deep(true).expect_throw("1");
        mogwai::web::body().append_child(&wrapper);
        let wrapper = wrapper.dyn_into::<web_sys::Element>().expect_throw("2.1");
        let wrapper = wrapper.dyn_into::<web_sys::Element>().expect_throw("2.2");
        let key_attrs = wrapper
            .query_selector_all(".has_key_attr")
            .expect_throw("2.5");
        let key_text = wrapper
            .query_selector(".has_key_text")
            .expect_throw("3")
            .expect_throw("4")
            .first_child()
            .expect_throw("5")
            .dyn_into::<web_sys::Text>()
            .expect_throw("6");
        let model_text = wrapper
            .query_selector(".has_model_text")
            .expect_throw("7")
            .expect_throw("8")
            .first_child()
            .expect_throw("9")
            .dyn_into::<web_sys::Text>()
            .expect_throw("10");
        Self {
            key_attrs,
            key_text,
            model_text,
            wrapper,
        }
    }
}

impl RowView {
    pub fn id(&self) -> Str {
        self.key_text.text_content().unwrap_or_default().into()
    }

    pub fn set_label(&self, text: impl AsRef<str>) {
        self.model_text.set_text_content(Some(text.as_ref()));
    }

    pub fn set_id(&self, text: impl AsRef<str>) {
        let text = text.as_ref();
        for i in 0..self.key_attrs.length() {
            let node = self.key_attrs.get(i).unwrap_throw();
            let _ = node
                .dyn_ref::<web_sys::Element>()
                .unwrap_throw()
                .set_attribute("key", text);
        }
        self.key_text.set_data(text);
    }

    pub fn set_model(&self, model: &RowModel) {
        self.set_label(&model.label);
        self.set_id(model.id.to_string());
    }

    pub fn set_selected(&self, is_selected: bool) {
        self.wrapper
            .set_property("class", if is_selected { "danger" } else { "" });
    }

    pub fn node(&self) -> &web_sys::Element {
        &self.wrapper
    }

    /// Appends " !!!" to the end of the text.
    pub fn update_text(&self) {
        let _ = self.model_text.append_data(" !!!");
    }

    pub fn fast_label(&self) -> String {
        self.model_text.data()
    }
}

//! The row.
use mogwai_futura::web::prelude::*;

pub struct RowModel {
    pub id: usize,
    pub label: Str,
}

#[derive(Clone)]
pub struct RowView<V: View = Builder> {
    id_text: V::Text,
    model_text: V::Text,
    wrapper: V::Element<web_sys::Element>,
}

impl Default for RowView {
    fn default() -> Self {
        rsx! {
            let wrapper = tr(key = "0") {
                td(class="col-md-1"){
                    let id_text = ""
                }
                td(class="col-md-4"){
                    a(class = "lbl", key = "0") {
                        let model_text = ""
                    }
                }
                td(class="col-md-1"){
                    a(class="remove" ) {
                        span(
                            class="remove glyphicon glyphicon-remove",
                            key = "0",
                            aria_hidden="true"
                        ) {}
                    }
                }
                td(class="col-md-6"){ }
            }
        }
        Self {
            id_text,
            model_text,
            wrapper,
        }
    }
}

impl From<RowView> for RowView<Web> {
    fn from(
        RowView {
            id_text,
            model_text,
            wrapper,
        }: RowView,
    ) -> Self {
        Self {
            id_text: id_text.into(),
            model_text: model_text.into(),
            wrapper: wrapper.into(),
        }
    }
}

impl<V: View> RowView<V> {
    pub fn id(&self) -> Str {
        self.id_text.get_text()
    }

    pub fn set_label(&self, text: impl Into<Str>) {
        self.model_text.set_text(text);
    }

    pub fn set_id(&self, text: impl Into<Str>) {
        self.id_text.set_text(text);
    }

    pub fn set_model(&self, model: &RowModel) {
        self.set_label(&model.label);
        self.set_id(model.id.to_string());
    }

    pub fn set_selected(&self, is_selected: bool) {
        self.wrapper
            .set_property("class", if is_selected { "danger" } else { "" });
    }

    pub fn node(&self) -> &V::Element<web_sys::Element> {
        &self.wrapper
    }
}

impl RowView<Web> {
    /// Appends " !!!" to the end of the text.
    pub fn update_text(&self) {
        let _ = self.model_text.append_data(" !!!");
    }

    pub fn fast_id(&self) -> String {
        self.wrapper.get_attribute("key").unwrap()
    }

    pub fn fast_label(&self) -> String {
        self.model_text.data()
    }
}

//! The row.
use mogwai_futura::web::prelude::*;

pub struct RowModel {
    pub id: usize,
    pub label: Str,
}

#[derive(Clone)]
pub struct RowView<V: View> {
    wrapper: V::Element,
    id_text: V::Text,
    model_text: V::Text,
    lbl_key: V::Element,
    remove_key: V::Element,
}

impl<V: View> Default for RowView<V> {
    fn default() -> Self {
        rsx! {
            let wrapper = tr(key = "0") {
                td(class="col-md-1"){
                    let id_text = ""
                }
                td(class="col-md-4"){
                    let lbl_key = a(class = "lbl", key = "0") {
                        let model_text = ""
                    }
                }
                td(class="col-md-1"){
                    a(class="remove" ) {
                        let remove_key = span(
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
            lbl_key,
            remove_key,
        }
    }
}

impl<V: View> RowView<V> {
    pub fn id(&self) -> Str {
        self.id_text.get_text()
    }

    pub fn set_label(&self, text: impl AsRef<str>) {
        self.model_text.set_text(text);
    }

    pub fn set_id(&self, text: impl Into<Str>) {
        let text = text.into();
        self.id_text.set_text(text.clone());
        self.lbl_key.set_property("key", text.clone());
        self.remove_key.set_property("key", text.clone());
        self.wrapper.set_property("key", text)
    }

    pub fn set_model(&self, model: &RowModel) {
        self.set_label(&model.label);
        self.set_id(model.id.to_string());
    }

    pub fn set_selected(&self, is_selected: bool) {
        self.wrapper
            .set_property("class", if is_selected { "danger" } else { "" });
    }

    pub fn node(&self) -> &V::Element {
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

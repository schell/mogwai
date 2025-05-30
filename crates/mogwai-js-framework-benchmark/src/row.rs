//! The row.
use mogwai_futura::web::prelude::*;

pub struct RowModel {
    pub id: usize,
    pub label: Str,
}

#[derive(Clone, FromBuilder)]
pub struct RowView<V: View = Builder> {
    id_text: V::Text,
    model_text: V::Text,
    wrapper: V::Element<web_sys::HtmlElement>,
}

impl<V: View + ViewOps<web_sys::HtmlElement>> RowView<V> {
    fn blah() -> Self {
        let wrapper = V::Element::<web_sys::HtmlElement>::new("tr");
        let _tr_td = V::Element::<web_sys::Element>::new("td");
        let id_text = V::Text::new("");
        _tr_td.append_child(&id_text);
        _tr_td.set_property("class", "col-md-1");
        wrapper.append_child(&_tr_td);
        let _tr_td1 = V::Element::<web_sys::Element>::new("td");
        let _tr_td1_a = V::Element::<web_sys::Element>::new("a");
        let model_text = V::Text::new("");
        _tr_td1_a.append_child(&model_text);
        _tr_td1_a.set_property("class", "lbl");
        _tr_td1_a.set_property("key", "0");
        _tr_td1.append_child(&_tr_td1_a);
        _tr_td1.set_property("class", "col-md-4");
        wrapper.append_child(&_tr_td1);
        let _tr_td2 = V::Element::<web_sys::Element>::new("td");
        let _tr_td2_a = V::Element::<web_sys::Element>::new("a");
        let _tr_td2_a_span = V::Element::<web_sys::Element>::new("span");
        _tr_td2_a_span.set_property("class", "remove glyphicon glyphicon-remove");
        _tr_td2_a_span.set_property("key", "0");
        _tr_td2_a_span.set_property("aria-hidden", "true");
        _tr_td2_a.append_child(&_tr_td2_a_span);
        _tr_td2_a.set_property("class", "remove");
        _tr_td2.append_child(&_tr_td2_a);
        _tr_td2.set_property("class", "col-md-1");
        wrapper.append_child(&_tr_td2);
        let _tr_td3 = V::Element::<web_sys::Element>::new("td");
        _tr_td3.set_property("class", "col-md-6");
        wrapper.append_child(&_tr_td3);
        wrapper.set_property("key", "0");
        Self {
            id_text,
            model_text,
            wrapper,
        }
    }
}

impl<V: View> Default for RowView<V> {
    fn default() -> Self {
        rsx! {
            let wrapper: web_sys::HtmlElement = tr(key = "0") {
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

fn test() {
    let view = RowView::<Web>::default();
    view.wrapper;
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

    pub fn node(&self) -> &V::Element<web_sys::HtmlElement> {
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

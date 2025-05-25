//! The "button clicks" UI.
use mogwai_futura::web::prelude::*;

pub enum ButtonClickEvent {
    Clicked,
    // For TUI
    Quit,
}

pub struct ButtonClicks {
    pub clicks: u32,
}

pub trait ButtonClicksInterface {
    fn title(&self) -> &impl ViewText;
    fn description(&self) -> &impl ViewText;
    fn get_next_event(&self) -> impl Future<Output = ButtonClickEvent>;
}

impl ButtonClicks {
    pub async fn run(&mut self, interface: impl ButtonClicksInterface) {
        interface.title().set_text("Button clicking demo.");

        loop {
            match interface.get_next_event().await {
                ButtonClickEvent::Clicked => {
                    self.clicks += 1;
                    interface
                        .description()
                        .set_text(format!("{} clicks.", self.clicks));
                }
                ButtonClickEvent::Quit => {
                    return;
                }
            }
        }
    }
}

pub struct Label<V: View = Builder> {
    wrapper: V::Element<web_sys::HtmlElement>,
    title: V::Text<web_sys::Text>,
}

impl Default for Label {
    fn default() -> Self {
        let wrapper = ElementBuilder::new("h2");
        let title = TextBuilder::new("Label");
        wrapper.append_child(&title);
        Label { wrapper, title }
    }
}

impl<V> ViewNode for Label<V>
where
    V: View,
{
    type Parent<T> = V::Element<T>;

    fn append_to_parent(&self, parent: impl AsRef<Self::Parent>) {
        parent.append_child(&self.wrapper);
    }
}

impl From<Label> for Label<Web> {
    fn from(value: Label) -> Self {
        Label {
            wrapper: Web::build_element(value.wrapper),
            title: Web::build_text(value.title),
        }
    }
}

pub struct ButtonClicksView<V: View = Builder> {
    pub wrapper: V::Element<web_sys::HtmlElement>,
    text: V::Text<web_sys::Text>,
    label: Label<V>,
    button_click: V::EventListener<EventListener>,
}

impl<V: View> ButtonClicksInterface for ButtonClicksView<V> {
    fn title(&self) -> &impl ViewText {
        &self.label.title
    }

    fn description(&self) -> &impl ViewText {
        &self.text
    }

    async fn get_next_event(&self) -> ButtonClickEvent {
        self.button_click.next().await;
        ButtonClickEvent::Clicked
    }
}

impl Default for ButtonClicksView {
    fn default() -> Self {
        let wrapper = ElementBuilder::new("div");
        wrapper.set_property("id", "buttonwrapper");
        let label = Label::default();
        wrapper.append_child(&label);
        let button = ElementBuilder::new("button");
        button.set_style("cursor", "pointer");
        let button_click = button.listen("click");
        wrapper.append_child(&button);
        let p = ElementBuilder::new("p");
        let text = TextBuilder::new("Click me.");
        p.append_child(&text);
        Self {
            wrapper,
            text,
            label,
            button_click,
        }
    }
}

impl From<ButtonClicksView> for ButtonClicksView<Web> {
    fn from(value: ButtonClicksView) -> Self {
        Self {
            wrapper: Web::build_element(value.wrapper),
            text: Web::build_text(value.text),
            label: value.label.into(),
            button_click: Web::build_listener(value.button_click),
        }
    }
}

impl ButtonClicksView<Web> {
    pub fn web(mut model: ButtonClicks) -> Result<(), wasm_bindgen::JsValue> {
        let view: ButtonClicksView<Web> = ButtonClicksView::default().into();
        let body = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .body()
            .unwrap();
        web_sys::Node::append_child(&body, &view.wrapper).unwrap();
        wasm_bindgen_futures::spawn_local(async move { model.run(view).await });
        Ok(())
    }
}

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
        log::info!("running the button clicks loop");

        interface.title().set_text("Button clicking demo.");

        loop {
            match interface.get_next_event().await {
                ButtonClickEvent::Clicked => {
                    log::info!("got a click");
                    self.clicks += 1;
                    interface
                        .description()
                        .set_text(format!("{} clicks.", self.clicks));
                }
                ButtonClickEvent::Quit => {
                    log::info!("quitting");
                    return;
                }
            }
        }
    }
}

#[derive(Clone, ViewChild)]
pub struct Label<V: View = Builder> {
    #[child]
    wrapper: V::Element<web_sys::HtmlElement>,
    title: V::Text,
}

impl Default for Label {
    fn default() -> Self {
        let wrapper = ElementBuilder::new("h2");
        let title = TextBuilder::new("Label");
        wrapper.append_child(&title);
        Label { wrapper, title }
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

#[derive(Clone)]
pub struct ButtonClicksView<V: View = Builder> {
    pub wrapper: V::Element<web_sys::HtmlElement>,
    text: V::Text,
    label: Label<V>,
    pub button_click: V::EventListener,
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
        log::info!("building the view");
        let view: ButtonClicksView<Web> = ButtonClicksView::default().into();
        log::info!("adding the view");
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

impl Default for ButtonClicksView {
    fn default() -> Self {
        rsx! {
            let wrapper = div(id = "buttonwrapper") {
                let label = {Label::default()}
                button(style:cursor = "pointer", on:click = button_click) {
                    p() {
                        let text = "Click me."
                    }
                }
            }
        }
        Self {
            wrapper,
            text,
            label,
            button_click,
        }
    }
}

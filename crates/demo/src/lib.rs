use mogwai_futura::web::prelude::*;

#[cfg(feature = "web")]
use wasm_bindgen::prelude::*;

pub enum ButtonClickEvent {
    Clicked,
    // For TUI
    Quit,
}

pub struct ButtonClicks {
    pub clicks: u32,
}

pub trait ButtonClicksInterface: Default {
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

impl Default for ButtonClicksView {
    fn default() -> Self {
        let wrapper = ElementBuilder::new("div");
        wrapper.set_property("id", "buttonwrapper");
        let label = Label::default();
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

#[cfg(feature = "web")]
#[wasm_bindgen(start)]
fn web_run() -> Result<(), wasm_bindgen::JsValue> {
    use web::ButtonClicksWeb;

    let mut model = ButtonClicks { clicks: 0 };

    let view = ButtonClicksWeb::default();
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .body()
        .unwrap()
        .append_child(&view.wrapper)
        .unwrap();
    wasm_bindgen_futures::spawn_local(async move { model.run(view).await });
    Ok(())
}

#[cfg(feature = "web")]
pub mod web {
    use mogwai_futura::macros::rsx_web;
    use mogwai_futura::web::event::EventListener;

    use super::*;

    pub struct LabelFieldWeb {
        wrapper: web_sys::HtmlElement,
        title: web_sys::Text,
    }

    impl Default for LabelFieldWeb {
        fn default() -> Self {
            rsx_web! {
                let wrapper: web_sys::HtmlElement = h2() {
                    let title = "Label"
                }
            }

            Self { wrapper, title }
        }
    }

    impl AsRef<web_sys::Node> for LabelFieldWeb {
        fn as_ref(&self) -> &web_sys::Node {
            self.wrapper.as_ref()
        }
    }

    pub struct ButtonClicksWeb {
        pub wrapper: web_sys::HtmlElement,
        text: web_sys::Text,
        label: LabelFieldWeb,
        button_click: EventListener,
    }

    impl Default for ButtonClicksWeb {
        fn default() -> Self {
            rsx_web! {
                let wrapper:web_sys::HtmlElement =
                    div(id = "buttonwrapper") {
                        let label = {LabelFieldWeb::default()}
                        button(
                            style:cursor = "pointer",
                            on:click = button_click
                        ) {
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

    impl ButtonClicksInterface for ButtonClicksWeb {
        fn title(&self) -> &impl ViewText {
            &self.label.title
        }

        fn description(&self) -> &impl ViewText {
            &self.text
        }

        async fn get_next_event(&self) -> ButtonClickEvent {
            let _ev = self.button_click.next().await;
            ButtonClickEvent::Clicked
        }
    }
}

#[cfg(feature = "tui")]
pub mod tui {
    use super::*;
    use mogwai_futura::{Str, sync::Shared};

    use crossterm::event::{self, EnableMouseCapture, Event};
    use ratatui::{
        Frame,
        layout::{Constraint, Layout, Margin, Position},
        style::Stylize,
        text::Line,
        widgets::Block,
    };

    #[derive(Clone)]
    struct ButtonClicksTui {
        label: Shared<Str>,
        clicks_text: Shared<Str>,
        channel: (async_channel::Sender<Event>, async_channel::Receiver<Event>),
        should_quit: Shared<bool>,
    }

    impl Default for ButtonClicksTui {
        fn default() -> Self {
            ButtonClicksTui {
                label: Shared::new("Label".into()),
                clicks_text: Shared::new("Click me.".into()),
                channel: async_channel::bounded(1),
                should_quit: Shared::new(false),
            }
        }
    }

    impl ButtonClicksInterface for ButtonClicksTui {
        fn title(&self) -> &impl ViewText {
            &self.label
        }

        fn description(&self) -> &impl ViewText {
            &self.clicks_text
        }

        async fn get_next_event(&self) -> ButtonClickEvent {
            loop {
                let ev = self.channel.1.recv().await.unwrap();
                if let Some(event) = self.handle_event(ev) {
                    return event;
                }
            }
        }
    }

    impl ButtonClicksTui {
        fn handle_event(&self, ev: Event) -> Option<ButtonClickEvent> {
            log::info!("got event: {ev:#?}");
            match ev {
                Event::Key(key) => {
                    key.code.is_esc().then_some(())?;
                    self.should_quit.set(true);
                    None
                }
                Event::Mouse(mouse_event) => match mouse_event.kind {
                    event::MouseEventKind::Down(event::MouseButton::Left) => {
                        Some(ButtonClickEvent::Clicked)
                    }
                    _ => None,
                },
                _ => None,
            }
        }

        fn draw(&self, frame: &mut Frame) {
            let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
            let [title_area, body_area] = vertical.areas(frame.area());

            let label_text = self.label.get();
            let title = Line::from(label_text.as_str()).centered().bold();
            frame.render_widget(title, title_area);

            let click_text = self.clicks_text.get();
            let button = Block::bordered().title(click_text.as_str());
            let button_area =
                body_area.inner(Margin::new(body_area.width / 3, body_area.height / 3));
            frame.render_widget(button, button_area);
        }
    }

    pub fn run() {
        simplelog::WriteLogger::init(
            simplelog::LevelFilter::Info,
            simplelog::Config::default(),
            std::fs::File::create("log.log").unwrap(),
        )
        .unwrap();
        log::info!("starting");

        let mut terminal = ratatui::init();
        let mut model = ButtonClicks { clicks: 0 };
        let view = ButtonClicksTui::default();

        // Run the logic in a separate thread
        std::thread::spawn({
            let view = view.clone();
            move || {
                futures_lite::future::block_on(model.run(view));
            }
        });

        terminal.draw(|frame| view.draw(frame)).unwrap();
        loop {
            crossterm::execute!(std::io::stdout(), EnableMouseCapture).unwrap();
            let has_event = event::poll(std::time::Duration::from_secs_f32(1.0 / 12.0)).unwrap();
            if has_event {
                terminal.set_cursor_position(Position::ORIGIN).unwrap();
                terminal.draw(|frame| view.draw(frame)).unwrap();
                let mut pos = terminal.get_cursor_position().unwrap();
                pos.x = 0;
                terminal.set_cursor_position(pos).unwrap();

                let ev = event::read().unwrap();
                view.channel.0.send_blocking(ev).unwrap();
            }
            crossterm::execute!(std::io::stdout(), EnableMouseCapture).unwrap();

            if *view.should_quit.get() {
                terminal.clear().unwrap();
                return;
            }
        }
    }
}

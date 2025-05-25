#[cfg(feature = "web")]
use wasm_bindgen::prelude::*;

pub mod button_clicks;

#[cfg(feature = "web")]
#[wasm_bindgen(start)]
fn web_run() -> Result<(), wasm_bindgen::JsValue> {
    button_clicks::ButtonClicksView::web(button_clicks::ButtonClicks { clicks: 0 })
}

#[cfg(feature = "tui")]
pub mod tui {
    use mogwai_futura::prelude::*;
    use mogwai_futura::{Str, sync::Shared};

    use crossterm::event::{self, EnableMouseCapture, Event};
    use ratatui::{
        Frame,
        layout::{Constraint, Layout, Margin, Position},
        style::Stylize,
        text::Line,
        widgets::Block,
    };

    use crate::button_clicks::*;

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

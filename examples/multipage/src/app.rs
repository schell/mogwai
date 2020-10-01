use crate::routes;
use mogwai::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    Home,
    NotFound,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    Render(Route),
    RenderClicks(i32),
    SkipRender,
}

#[derive(Debug)]
pub struct App {
    click_count: i32,
    current_route: Route,
}

impl App {
    pub fn new() -> Self {
        App {
            click_count: 0,
            current_route: Route::Home,
        }
    }

    fn render(route: Route) -> View<HtmlElement> {
        log::trace!("#render({:?})", &route);
        let content = match route {
            // pub fn home() -> View<HtmlElement>
            Route::Home => routes::home(),
            // pub fn not_found() -> View<HtmlElement>
            Route::NotFound => routes::not_found(),
        };
        view! {
            <main>
                {content}
            </main>
        }
    }
}

impl Component for App {
    type ModelMsg = Route;
    type ViewMsg = Out;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &Route, tx_view: &Transmitter<Out>, _sub: &Subscriber<Route>) {
        log::trace!("#update(msg: {:?}, current: {:?})", msg, self.current_route);
        if self.current_route == *msg {
            tx_view.send(&Out::SkipRender);
        } else {
            self.current_route = msg.clone();
            tx_view.send(&Out::Render(self.current_route));
        }
        self.click_count += 1;
        tx_view.send(&Out::RenderClicks(self.click_count));
    }

    fn view(&self, tx: &Transmitter<Route>, rx: &Receiver<Out>) -> ViewBuilder<HtmlElement> {
        log::trace!("{:?}", &self);
        let rx_text = rx.branch_filter_map(|msg| match msg {
            Out::RenderClicks(count) => Some(format!("{} times", count)),
            _ => None,
        });
        let rx_main = rx.branch_filter_map(|msg| {
            log::trace!("rx_main({:?})", msg);
            match msg {
                Out::Render(route) => Some(App::render(*route)),
                _ => None,
            }
        });
        let contents = ViewBuilder {
            element: Some("main".into()),
            replaces: vec![rx_main],
            ..ViewBuilder::default()
        };
        builder! {
            <div class="root">
                <nav>
                    <button on:click=tx.contra_map(|_| Route::Home)>
                        "Home"
                    </button>
                    <button on:click=tx.contra_map(|_| Route::NotFound)>
                        "Not Found"
                    </button>
                </nav>
                <p>{("0 times", rx_text)}</p>
                {contents}
            </div>
        }
    }
}

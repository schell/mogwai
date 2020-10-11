use crate::routes;
use mogwai::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Game { game_id: String },
    GameList,
    Home,
    NotFound,
}

impl From<Route> for View<HtmlElement> {
    fn from(route: Route) -> Self {
        ViewBuilder::from(route).into()
    }
}

impl From<Route> for ViewBuilder<HtmlElement> {
    fn from(route: Route) -> Self {
        match route {
            Route::Game { game_id } => routes::game(game_id),
            Route::GameList => Gizmo::from(routes::GameList::default()).view_builder(),
            Route::Home => routes::home(),
            Route::NotFound => routes::not_found(),
        }
    }
}

#[derive(Clone)]
pub enum Out {
    Render {
        route: Route,
        view_builder: ViewBuilder<HtmlElement>,
    },
    RenderClicks(i32),
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
}

impl Component for App {
    type ModelMsg = Route;
    type ViewMsg = Out;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &Route, tx_view: &Transmitter<Out>, _sub: &Subscriber<Route>) {
        if self.current_route != *msg {
            self.current_route = msg.clone();
            tx_view.send(&Out::Render {
                route: self.current_route.clone(),
                view_builder: ViewBuilder::from(self.current_route.clone()),
            });
        }
        self.click_count += 1;
        tx_view.send(&Out::RenderClicks(self.click_count));
    }

    #[allow(unused_braces)]
    fn view(&self, tx: &Transmitter<Route>, rx: &Receiver<Out>) -> ViewBuilder<HtmlElement> {
        let rx_text = rx.branch_filter_map(|msg| match msg {
            Out::RenderClicks(count) if count == &1 => Some(format!("{} time", count)),
            Out::RenderClicks(count) => Some(format!("{} times", count)),
            _ => None,
        });
        let rx_main = rx.branch_filter_map(|msg| match msg {
            Out::Render {
                route: _,
                view_builder,
            } => Some(View::from(view_builder.clone())),
            _ => None,
        });
        let mut contents = ViewBuilder::from(self.current_route.clone());
        contents.replaces = vec![rx_main];
        builder! {
            <div class="root">
                <p>{("0 times", rx_text)}</p>
                <nav>
                    <a
                        href="/"
                        style="margin-right: 15px;"
                        on:click=tx.contra_map(|e: &Event| {
                            e.prevent_default();
                            Route::Home
                        })
                    >
                        "Home"
                    </a>
                    <a
                        href="/game"
                        style="margin-right: 15px;"
                        on:click=tx.contra_map(|e: &Event| {
                            e.prevent_default();
                            Route::GameList
                        })
                    >
                        "Games"
                    </a>
                    <a
                        href="/404"
                        style="margin-right: 15px;"
                        on:click=tx.contra_map(|e: &Event| {
                            e.prevent_default();
                            Route::NotFound
                        })
                    >
                        "Not Found"
                    </a>
                </nav>
                {contents}
            </div>
        }
    }
}

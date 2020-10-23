use crate::{route_dispatch, Route};
use mogwai::prelude::*;

#[derive(Copy, Clone, Debug)]
pub enum Out {
    Render { route: Route },
    RenderClicks(i32),
}

#[derive(Debug)]
pub struct App {
    click_count: i32,
    current_route: Route,
}

impl App {
    pub fn gizmo(initial_route: Route) -> Gizmo<Self> {
        let app = App {
            click_count: 0,
            current_route: initial_route,
        };
        Gizmo::from(app)
    }
}

impl Component for App {
    type ModelMsg = Route;
    type ViewMsg = Out;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &Route, tx_view: &Transmitter<Out>, _sub: &Subscriber<Route>) {
        if self.current_route != *msg {
            self.current_route = *msg;
            tx_view.send(&Out::Render {
                route: self.current_route,
            });
            route_dispatch::push_state(*msg);
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
        let rx_main = rx.branch_filter_map(move |msg| match msg {
            Out::Render { route } => Some(Patch::Replace {
                index: 0,
                value: ViewBuilder::from(*route),
            }),
            _ => None,
        });
        let contents = ViewBuilder::from(self.current_route);
        builder! {
            <div id="root" class="root">
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
                <slot patch:children=rx_main>
                    {contents}
                </slot>
            </div>
        }
    }
}

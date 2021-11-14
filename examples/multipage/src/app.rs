use crate::{route_dispatch, Route};
use mogwai::prelude::*;

#[derive(Copy, Clone, Debug)]
pub enum Out {
    Render { route: Route },
    RenderClicks(i32),
}

impl Out {
    fn maybe_patch_route(&self) -> Option<ListPatch<ViewBuilder<Dom>>> {
        if let Out::Render { route } = self {
            Some(ListPatch::replace(0, route.into()))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct App {
    click_count: i32,
    current_route: Route,
}

impl App {
    pub fn component(initial_route: Route) -> Component<Dom> {
        let app = App {
            click_count: 0,
            current_route: initial_route,
        };
        let (tx_logic, rx_logic) = broadcast::bounded(1);
        let (tx_view, rx_view) = broadcast::bounded(1);
        Component::from(app.view(tx_logic, rx_view))
            .with_logic(app.into_logic(rx_logic, tx_view))
    }

    async fn into_logic(mut self, mut rx_logic: broadcast::Receiver<Route>, tx_view: broadcast::Sender<Out>) {
        loop {
            match rx_logic.next().await {
                Some(route) => {
                    if self.current_route != route {
                        self.current_route = route;
                        tx_view.broadcast(Out::Render {
                            route: self.current_route,
                        }).await.unwrap();
                        route_dispatch::push_state(route);
                    }
                    self.click_count += 1;
                    tx_view.broadcast(Out::RenderClicks(self.click_count)).await.unwrap();
                }
                None => break,
            }
        }
    }

    fn view(&self, tx: broadcast::Sender<Route>, rx: broadcast::Receiver<Out>) -> ViewBuilder<Dom> {
        let rx_text = rx.clone().filter_map(|msg| async move { match msg {
            Out::RenderClicks(count) if count == 1 => Some(format!("{} time", count)),
            Out::RenderClicks(count) => Some(format!("{} times", count)),
            _ => None,
        }});
        builder! {
            <div id="root" class="root">
                <p>{("0 times", rx_text)}</p>
                <nav>
                    <a
                        href="/"
                        style="margin-right: 15px;"
                        on:click=tx.sink().contra_map(|e: Event| {
                            e.prevent_default();
                            Route::Home
                        })
                    >
                        "Home"
                    </a>
                    <a
                        href="/404"
                        style="margin-right: 15px;"
                        on:click=tx.sink().contra_map(|e: Event| {
                            e.prevent_default();
                            Route::NotFound
                        })
                    >
                        "Not Found"
                    </a>
                </nav>
                <slot patch:children=rx.clone().filter_map(|out| async move { out.maybe_patch_route() })>
                    {ViewBuilder::from(&self.current_route)}
                </slot>
            </div>
        }
    }
}

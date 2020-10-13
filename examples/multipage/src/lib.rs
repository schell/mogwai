mod api;
mod app;
mod components;
mod routes;

use crate::app::App;
use mogwai::prelude::*;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Route {
    Game { game_id: api::GameId },
    GameList,
    Home,
    NotFound,
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not init console_log");

    if cfg!(debug_assertions) {
        log::trace!("Hello from debug mogwai-multipage");
    } else {
        log::trace!("Hello from release mogwai-multipage");
    }

    // Create our app's view by hydrating a gizmo from an initial state
    let root: Gizmo<App> = App::gizmo();

    // Hand the app's view ownership to the window so it never
    // goes out of scope
    View::from(root).run()
}

/// `RouteDispatcher` is a reference counting container for a `Transmitter<Route>`.
/// This means cloning a `RouteDispatcher` increases the reference count to the
/// `Transmitter` and when the clone is dropped from scope the reference count
/// is decreased (see `Rc`).
#[derive(Clone)]
pub struct RouteDispatcher {
    tx: std::rc::Rc<Transmitter<Route>>,
}

impl RouteDispatcher {
    /// Create a new `RouteDispatcher` from the given `Transmitter`.
    fn new(tx: Transmitter<Route>) -> Self {
        RouteDispatcher {
            tx: std::rc::Rc::new(tx),
        }
    }

    /// Dispatch the given `Route`.
    pub fn navigate(&self, route: Route) {
        self.tx.send(&route);
    }

    /// Create a `ViewBuilder` for the given `Route`. The `ViewBuilder` will be
    /// given access to the `Transmitter`.
    fn view_builder(&self, route: Route) -> ViewBuilder<HtmlElement> {
        match route {
            Route::Game { game_id } => routes::game(game_id),
            Route::GameList => {
                let component = routes::GameList::new(self.tx.clone(), vec![]);
                Gizmo::from(component).view_builder()
            }
            Route::Home => routes::home(),
            Route::NotFound => routes::not_found(),
        }
    }
}

impl std::fmt::Debug for RouteDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteDispatcher").finish()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

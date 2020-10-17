mod app;
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
    Home,
    NotFound,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn view<T>(path: T) -> String
where
    T: AsRef<str>,
{
    let initial_route: Route = path.into();
    // Create our app's view by hydrating a gizmo from an initial state
    let root: Gizmo<App> = App::gizmo(initial_route);
    let builder = root.view_builder();
    let view = View::from(builder);
    view.html_string()
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

    let initial_route = Route::from(utils::window().location().pathname().unwrap_throw());
    // Create our app's view by hydrating a gizmo from an initial state
    let root: Gizmo<App> = App::gizmo(initial_route);

    // Hand the app's view ownership to the window so it never
    // goes out of scope
    let hydrator = mogwai_hydrator::Hydrator::from(root.view_builder());
    match View::try_from(hydrator) {
        Ok(view) => view.run(),
        Err(err) => Err(JsValue::from(err.to_string())),
    }
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
    pub fn push_state(&self, route: Route) {
        let window = mogwai::utils::window();
        match window.history() {
            Ok(history) => {
                let state = JsValue::from("");
                let push_result =
                    history.push_state_with_url(&state, "", Some(&format!("{}", route)));
                if let Err(error) = push_result {
                    ::log::debug!("{:?}", error);
                }
            }
            Err(error) => ::log::debug!("{:?}", error),
        }
    }

    /// Create a `ViewBuilder` for the given `Route`. The `ViewBuilder` will be
    /// given access to the `Transmitter`.
    fn view_builder(&self, route: Route) -> ViewBuilder<HtmlElement> {
        match route {
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

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Route::Home => f.write_str("/"),
            Route::NotFound => f.write_str("/404"),
        }
    }
}

impl<T: AsRef<str>> From<T> for Route {
    fn from(path: T) -> Self {
        let s = path.as_ref();
        ::log::trace!("route try_from: {}", s);
        // remove the scheme, if it has one
        let paths: Vec<&str> = s.split("/").collect::<Vec<_>>();
        ::log::trace!("route paths: {:?}", paths);

        match paths.as_slice() {
            [""] => Route::Home,
            ["", ""] => Route::Home,
            _ => Route::NotFound,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

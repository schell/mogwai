#![allow(unused_braces)]
use log::{trace, Level};
use mogwai::prelude::*;
use std::{panic, convert::TryFrom};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::HashChangeEvent;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Here we enumerate all our app's routes.
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Home,
    Settings,
    Profile {
        username: String,
        is_favorites: bool,
    },
}

/// We'll use TryFrom::try_from to convert the window's url hash into a Route.
impl TryFrom<&str> for Route {
    type Error = String;

    fn try_from(s: &str) -> Result<Route, String> {
        trace!("route try_from: {}", s);
        // remove the scheme, if it has one
        let hash_split = s.split("#").collect::<Vec<_>>();
        let after_hash = match hash_split.as_slice() {
            [_, after] => Ok(after),
            _ => Err(format!("route must have a hash: {}", s)),
        }?;

        let paths: Vec<&str> = after_hash.split("/").collect::<Vec<_>>();
        trace!("route paths: {:?}", paths);

        match paths.as_slice() {
            [""] => Ok(Route::Home),
            ["", ""] => Ok(Route::Home),
            ["", "settings"] => Ok(Route::Settings),
            // Here you can see that profile may match two different routes -
            // '#/profile/{username}' and '#/profile/{username}/favorites'
            ["", "profile", username] => Ok(Route::Profile {
                username: username.to_string(),
                is_favorites: false,
            }),
            ["", "profile", username, "favorites"] => Ok(Route::Profile {
                username: username.to_string(),
                is_favorites: true,
            }),
            r => Err(format!("unsupported route: {:?}", r)),
        }
    }
}

#[cfg(test)]
mod test_route_try_from {
    use super::*;

    #[test]
    fn can_convert_string_to_route() {
        let s = "https://localhost:8080/#/";
        assert_eq!(Route::try_from(s), Ok(Route::Home));
    }
}

/// Convert the route into its hashed string.
/// This should match the inverse conversion in TryFrom above.
impl From<Route> for String {
    fn from(route: Route) -> String {
        match route {
            Route::Home => "#/".into(),
            Route::Settings => "#/settings".into(),
            Route::Profile {
                username,
                is_favorites,
            } => {
                if is_favorites {
                    format!("#/profile/{}/favorites", username)
                } else {
                    format!("#/profile/{}", username)
                }
            }
        }
    }
}

/// We can convert a route into a ViewBuilder in order to embed it in a component.
/// This is just a suggestion for this specific example. The general idea is
/// to use the route to inform your app that it needs to change the page. This
/// is just one of many ways to accomplish that.
impl From<&Route> for ViewBuilder<Dom> {
    fn from(route: &Route) -> Self {
        match route {
            Route::Home => builder! {
                <main>
                    <h1>"Welcome to the homepage"</h1>
                </main>
            },
            Route::Settings => builder! {
                <main>
                    <h1>"Update your settings"</h1>
                </main>
            },
            Route::Profile {
                username,
                is_favorites,
            } => builder! {
                <main>
                    <h1>{username}"'s Profile"</h1>
                    {if *is_favorites {
                        Some(builder!{
                            <h2>"Favorites"</h2>
                        })
                    } else {
                        None
                    }}
                </main>
            },
        }
    }
}

/// Here we'll define some helpers for displaying information about the current route.
impl Route {
    pub fn nav_home_class(&self) -> String {
        match self {
            Route::Home => "nav-link active",
            _ => "nav-link",
        }
        .to_string()
    }

    pub fn nav_settings_class(&self) -> String {
        match self {
            Route::Settings { .. } => "nav-link active",
            _ => "nav-link",
        }
        .to_string()
    }

    pub fn nav_profile_class(&self) -> String {
        match self {
            Route::Profile { .. } => "nav-link active",
            _ => "nav-link",
        }
        .to_string()
    }
}

#[derive(Clone)]
enum AppModel {
    HashChange(String),
}

#[derive(Clone)]
struct AppError(String);

async fn logic(
    mut route: Route,
    mut rx_logic: broadcast::Receiver<AppModel>,
    tx_view: broadcast::Sender<AppError>,
    mut tx_route_patch: mpsc::Sender<ListPatch<ViewBuilder<Dom>>>,
) {
    loop {
        match rx_logic.next().await {
            Some(AppModel::HashChange(hash)) => {
                // When we get a hash change, attempt to convert it into one of our routes
                match Route::try_from(hash.as_str()) {
                    // If we can't, let's send an error message to the view
                    Err(msg) => {
                        tx_view.broadcast(AppError(msg)).await.unwrap();
                    }
                    // If we _can_, create a new view from the route and send a patch message to
                    // the view
                    Ok(new_route) => {
                        trace!("got new route: {:?}", new_route);
                        if new_route != route {
                            let builder = ViewBuilder::from(&new_route);
                            route = new_route;
                            let patch = ListPatch::replace(2, builder);
                            tx_route_patch.send(patch).await.unwrap();
                        }
                        tx_view.broadcast(AppError("".to_string())).await.unwrap();
                    }
                }
            }
            None => break,
        }
    }
}

fn view(
    route: &Route,
    tx_logic: broadcast::Sender<AppModel>,
    rx_view: broadcast::Receiver<AppError>,
    rx_route_patch: mpsc::Receiver<ListPatch<ViewBuilder<Dom>>>,
) -> ViewBuilder<Dom> {
    let username: String = "Reasonable-Human".into();
    builder! {
        <slot
            window:hashchange=tx_logic.contra_filter_map(|ev: DomEvent| {
                let ev = ev.browser_event()?;
                let hev = ev.dyn_ref::<HashChangeEvent>().unwrap().clone();
                let hash = hev.new_url();
                Some(AppModel::HashChange(hash))
            })
            patch:children=rx_route_patch>
            <nav>
                <ul>
                    <li class=route.nav_home_class()>
                        <a href=String::from(Route::Home)>"Home"</a>
                    </li>
                    <li class=route.nav_settings_class()>
                        <a href=String::from(Route::Settings)>"Settings"</a>
                    </li>
                    <li class=route.nav_settings_class()>
                        <a href=String::from(Route::Profile {
                            username: username.clone(),
                            is_favorites: true
                        })>
                            {format!("{}'s Profile", username)}
                        </a>
                    </li>
                </ul>
            </nav>
            <pre>{("", rx_view.map(|AppError(msg)| msg))}</pre>
            {route}
        </slot>
    }
}

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    mogwai::spawn(async {
        let route = Route::Home;
        let (tx_logic, rx_logic) = broadcast::bounded(1);
        let (tx_view, rx_view) = broadcast::bounded(1);
        let (tx_route_patch, rx_route_patch) = mpsc::bounded(1);
        let component = Component::from(view(&route, tx_logic, rx_view, rx_route_patch))
            .with_logic(logic(route, rx_logic, tx_view, tx_route_patch));
        let view = component.build().unwrap();

        if let Some(id) = parent_id {
            let parent = mogwai::dom::utils::document()
                .visit_js(|doc: web_sys::Document| doc.get_element_by_id(&id))
                .map(Dom::wrap_js)
                .unwrap();
            view.run_in_container(&parent)
        } else {
            view.run()
        }
    });

    Ok(())
}

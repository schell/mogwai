#![allow(unused_braces)]
use log::{trace, Level};
//use mogwai_dom::core::channel::{broadcast, mpsc};
use mogwai_dom::prelude::*;
use std::{convert::TryFrom, panic};
use wasm_bindgen::{prelude::*, JsCast};
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
impl From<&Route> for ViewBuilder {
    fn from(route: &Route) -> Self {
        match route {
            Route::Home => html! {
                <main>
                    <h1>"Welcome to the homepage"</h1>
                </main>
            },
            Route::Settings => html! {
                <main>
                    <h1>"Update your settings"</h1>
                </main>
            },
            Route::Profile {
                username,
                is_favorites,
            } => html! {
                <main>
                    <h1>{username}"'s Profile"</h1>
                    {if *is_favorites {
                        Some(html!{
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

fn app(starting_route: Route) -> ViewBuilder {
    let username: String = "Reasonable-Human".into();

    let mut input_route = Input::<Route>::default();
    let mut input_error_msg = Input::<String>::default();

    let output_window_hashchange = Output::<JsDomEvent>::default();

    let builder = rsx! {
        slot(
            //window:hashchange = tx_logic.contra_filter_map(|ev: JsDomEvent| {
            //}),
            window:hashchange = output_window_hashchange.sink(),
            patch:children = input_route
                .stream()
                .unwrap()
                .map(|r| ListPatch::replace(2, ViewBuilder::from(&r)))
        ) {
            nav() {
                ul() {
                    li(class = starting_route.nav_home_class()) {
                        a(href = String::from(Route::Home)) { "Home" }
                    }
                    li(class = starting_route.nav_settings_class()) {
                        a(href = String::from(Route::Settings)) { "Settings" }
                    }
                    li(class = starting_route.nav_settings_class()) {
                        a(href = String::from(Route::Profile {
                            username: username.clone(),
                            is_favorites: true
                        })) {
                            {format!("{}'s Profile", username)}
                        }
                    }
                }
            }
            pre() { {("", input_error_msg.stream().unwrap())} }
            {&starting_route}
        }
    };
    builder.with_task(async move {
        let mut route = starting_route;
        while let Some(ev) = output_window_hashchange.get().await {
            let hash = {
                let ev = ev.browser_event().expect("not a browser event");
                let hev = ev.dyn_ref::<HashChangeEvent>().unwrap().clone();
                hev.new_url()
            };
            // When we get a hash change, attempt to convert it into one of our routes
            let msg = match Route::try_from(hash.as_str()) {
                // If we can't, let's send an error message to the view
                Err(msg) => msg,
                // If we _can_, create a new view from the route and send a patch message to
                // the view
                Ok(new_route) => {
                    trace!("got new route: {:?}", new_route);
                    if new_route != route {
                        route = new_route;
                        input_route.set(route.clone()).await.expect("could not set route");
                    }
                    // ...and clear any existing error message
                    String::new()
                }
            };
            input_error_msg
                .set(msg)
                .await
                .expect("could not set error msg");
        }
    })
}

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    // TODO: instantiate the route from the window location
    let route = Route::Home;
    let view = JsDom::try_from(app(route)).unwrap();

    if let Some(id) = parent_id {
        let parent = mogwai_dom::utils::document()
            .visit_as(|doc: &web_sys::Document| {
                JsDom::from_jscast(&doc.get_element_by_id(&id).unwrap())
            })
            .unwrap();
        view.run_in_container(parent)
    } else {
        view.run()
    }
    .unwrap();

    Ok(())
}

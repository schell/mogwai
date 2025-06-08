#![allow(unused_braces)]
use log::{trace, Level};
use mogwai::web::prelude::*;
use std::{convert::TryFrom, panic};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::HashChangeEvent;

/// Here we enumerate all our app's routes.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Route {
    #[default]
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

impl Route {
    fn view<V: View>(&self) -> V::Element {
        match self {
            Route::Home => {
                rsx! {
                    let el = main() {
                        h1() { "Welcome to the homepage" }
                    }
                }
                el
            }
            Route::Settings => {
                rsx! {
                    let el = main() {
                        h1() { "Update your settings" }
                    }
                }
                el
            }
            Route::Profile {
                username,
                is_favorites,
            } => {
                let favorites = if *is_favorites {
                    Some({
                        rsx! {
                            let h = h2() { "Favorites" }
                        }
                        h
                    })
                } else {
                    None
                };
                rsx! {
                    let el = main() {
                        h1() {{ format!("{username}'s Profile") }}
                        {favorites}
                    }
                }
                el
            }
        }
    }

    pub fn nav_home_class(&self) -> String {
        match self {
            Route::Home => "nav-link active",
            _ => "nav-link",
        }
        .to_string()
    }

    pub fn nav_settings_class(&self) -> String {
        match self {
            Route::Settings => "nav-link active",
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

#[derive(ViewChild)]
struct App<V: View> {
    #[child]
    wrapper: V::Element,
    on_window_hashchange: V::EventListener,
    error_text: V::Text,
    proxy_route: Proxy<Route>,
}

impl<V: View> Default for App<V> {
    fn default() -> Self {
        let mut proxy_route = Proxy::<Route>::default();

        rsx! {
            let wrapper = slot(window:hashchange = on_window_hashchange) {
                nav() {
                    ul() {
                        li(class = proxy_route(route => route.nav_home_class())) {
                            a(href = String::from(Route::Home)) { "Home" }
                        }
                        li(class = proxy_route(route => route.nav_settings_class())) {
                            a(href = String::from(Route::Settings)) { "Settings" }
                        }
                        li(class = proxy_route(route => route.nav_profile_class())) {
                            a(href = String::from(Route::Profile {
                                username: "Decent-Human".into(),
                                is_favorites: true
                            })) { "Profile" }
                        }
                    }
                }
                pre() {
                    let error_text = ""
                }
                {proxy_route(route => route.view::<V>())}
            }
        }

        Self {
            wrapper,
            on_window_hashchange,
            error_text,
            proxy_route,
        }
    }
}

impl App<Web> {
    async fn run_until_next(&mut self) {
        let ev = self.on_window_hashchange.next().await;

        let hash = {
            let hev = ev.dyn_ref::<HashChangeEvent>().unwrap().clone();
            hev.new_url()
        };

        // When we get a hash change, attempt to convert it into one of our routes
        let err_msg = match Route::try_from(hash.as_str()) {
            // If we can't, let's send an error message to the view
            Err(msg) => msg,
            // If we _can_, create a new view from the route and set the route on the
            // view(s)
            Ok(new_route) => {
                trace!("got new route: {:?}", new_route);
                self.proxy_route.set(new_route.clone());

                // ...and clear any existing error message
                String::new()
            }
        };

        self.error_text.set_data(&err_msg);
    }
}

#[wasm_bindgen]
pub fn run(parent_id: Option<String>) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap_throw();

    let mut app = App::<Web>::default();

    if let Some(id) = parent_id {
        let parent = mogwai::web::document()
            .get_element_by_id(&id)
            .unwrap_throw();
        parent.append_child(&app);
    } else {
        mogwai::web::body().append_child(&app);
    }

    wasm_bindgen_futures::spawn_local(async move {
        loop {
            app.run_until_next().await;
        }
    });
}

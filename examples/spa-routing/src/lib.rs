#![allow(unused_braces)]
use log::{trace, Level};
use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;
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

/// We can convert a route into a ViewBuilder in order to embed it in a gizmo.
/// This is just a suggestion for this specific example. The general idea is
/// to use the route to inform your app that it needs to change the page. This
/// is just one of many ways to accomplish that.
impl From<&Route> for ViewBuilder<HtmlElement> {
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

impl From<&Route> for View<HtmlElement> {
    fn from(route: &Route) -> Self {
        ViewBuilder::from(route).into()
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

struct App {
    route: Route,
}

#[derive(Clone)]
enum AppModel {
    HashChange(String),
}

#[derive(Clone)]
enum AppView {
    PatchPage(Patch<View<HtmlElement>>),
    Error(String),
}

impl AppView {
    fn error(&self) -> Option<String> {
        match self {
            AppView::Error(msg) => Some(msg.clone()),
            _ => None,
        }
    }

    /// If the message is a new route, convert it into a patch to replace the current main page.
    fn patch_page(&self) -> Option<Patch<View<HtmlElement>>> {
        match self {
            AppView::PatchPage(patch) => Some(patch.clone()),
            _ => None,
        }
    }
}

impl Component for App {
    type DomNode = HtmlElement;
    type ModelMsg = AppModel;
    type ViewMsg = AppView;

    fn update(&mut self, msg: &AppModel, tx: &Transmitter<AppView>, _sub: &Subscriber<AppModel>) {
        match msg {
            AppModel::HashChange(hash) => {
                // When we get a hash change, attempt to convert it into one of our routes
                match Route::try_from(hash.as_str()) {
                    // If we can't, let's send an error message to the view
                    Err(msg) => tx.send(&AppView::Error(msg)),
                    // If we _can_, create a new view from the route and send a patch message to
                    // the view
                    Ok(route) => {
                        if route != self.route {
                            let view = View::from(ViewBuilder::from(&route));
                            self.route = route;
                            tx.send(&AppView::PatchPage(Patch::Replace {
                                index: 2,
                                value: view,
                            }));
                        }
                    }
                }
            }
        }
    }

    fn view(&self, tx: &Transmitter<AppModel>, rx: &Receiver<AppView>) -> ViewBuilder<HtmlElement> {
        let username: String = "Reasonable-Human".into();
        builder! {
            <slot
                window:hashchange=tx.contra_filter_map(|ev:&Event| {
                    let hev = ev.dyn_ref::<HashChangeEvent>().unwrap().clone();
                    let hash = hev.new_url();
                    Some(AppModel::HashChange(hash))
                })
                patch:children=rx.branch_filter_map(AppView::patch_page)>
                <nav>
                    <ul>
                        <li class=self.route.nav_home_class()>
                            <a href=String::from(Route::Home)>"Home"</a>
                        </li>
                        <li class=self.route.nav_settings_class()>
                            <a href=String::from(Route::Settings)>"Settings"</a>
                        </li>
                        <li class=self.route.nav_settings_class()>
                            <a href=String::from(Route::Profile {
                                username: username.clone(),
                                is_favorites: true
                            })>
                                {format!("{}'s Profile", username)}
                            </a>
                        </li>
                    </ul>
                </nav>
                <pre>{rx.branch_filter_map(AppView::error)}</pre>
                {ViewBuilder::from(&self.route)}
            </slot>
        }
    }
}


#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let gizmo = Gizmo::from(App { route: Route::Home });
    let view = View::from(gizmo.view_builder());
    if let Some(id) = parent_id {
        let parent = utils::document()
            .get_element_by_id(&id)
            .unwrap();
        view.run_in_container(&parent)
    } else {
        view.run()
    }
}

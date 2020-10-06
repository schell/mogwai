use mogwai::prelude::utils;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

const API_URL: &'static str = "http://localhost:3000";

pub mod model {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
    pub enum GameStatus {
        OPEN,
        WON,
        LOST,
    }

    #[derive(Clone, Copy, Debug)]
    pub enum GetGameError {
        FetchError,
        ParseError,
        RequestCreateError,
        RequestHeaderSetError,
    }

    /// A struct to hold some data from the Game API.
    ///
    /// Note how we don't have to define every member -- serde will ignore extra
    /// data when deserializing
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct GameState {
        pub id: String,
        pub board: Vec<Vec<String>>,
        pub status: GameStatus,
    }
}

use model::*;
pub async fn get_game(game_id: String) -> Result<GameState, GetGameError> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let url = format!("{}/game/{}", API_URL, game_id);

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|_| GetGameError::RequestCreateError)?;

    request
        .headers()
        .set("Accept", "application/json")
        .map_err(|_| GetGameError::RequestHeaderSetError)?;

    let window = utils::window();
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| GetGameError::FetchError)?;

    // `resp_value` is a `Response` object.
    let resp: Response = resp_value.dyn_into().unwrap();

    // Convert this other `Promise` into a rust `Future`.
    let json = JsFuture::from(resp.json().map_err(|_| GetGameError::FetchError)?)
        .await
        .map_err(|_| GetGameError::FetchError)?;

    // Use serde to parse the JSON into a struct.
    json.into_serde().map_err(|_| GetGameError::ParseError)
}

use crate::Route;
use mogwai::prelude::*;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {}

type GameId = String;

pub struct GameList {
    dispatch: Rc<Transmitter<Route>>,
    game_ids: Rc<Vec<String>>,
}

#[derive(Clone, Debug)]
pub enum GameListModel {
    Navigate { game_id: GameId },
    ReplaceList { game_ids: Rc<Vec<String>> },
}

#[derive(Clone, Debug)]
pub struct GameListView {
    game_ids: Rc<Vec<String>>,
}

impl GameList {
    pub fn new(dispatch: Rc<Transmitter<Route>>, game_ids: Vec<String>) -> Self {
        Self {
            dispatch,
            game_ids: Rc::new(game_ids),
        }
    }

    fn game_ul(
        tx: &Transmitter<GameListModel>,
        game_ids: &Vec<String>,
    ) -> ViewBuilder<HtmlElement> {
        let mut game_ul = builder! { <ul /> };
        let game_links = game_ids
            .iter()
            .map(|game_id| GameList::game_li(&tx, game_id));
        for game_li in game_links {
            game_ul.with(game_li);
        }
        game_ul
    }

    #[allow(unused_braces)]
    fn game_li(tx: &Transmitter<GameListModel>, game_id: &String) -> ViewBuilder<HtmlElement> {
        let game_href = format!("/game/{}", game_id);
        let id = game_id.clone();
        let handler: Transmitter<Event> = tx.contra_map(move |e: &Event| {
            e.prevent_default();
            GameListModel::Navigate {
                game_id: id.clone(),
            }
        });
        builder! {
            <li>
                <a href=game_href on:click=handler>{game_id}</a>
            </li>
        }
    }
}

impl Component for GameList {
    type ModelMsg = GameListModel;
    type ViewMsg = GameListView;
    type DomNode = HtmlElement;

    fn update(
        &mut self,
        msg: &GameListModel,
        tx: &Transmitter<GameListView>,
        _sub: &Subscriber<GameListModel>,
    ) {
        match msg {
            GameListModel::ReplaceList { game_ids } => {
                self.game_ids = game_ids.clone();
                tx.send(&GameListView {
                    game_ids: self.game_ids.clone(),
                });
            }
            GameListModel::Navigate { game_id } => {
                self.dispatch.send(&Route::Game {
                    game_id: game_id.clone(),
                });
            }
        }
    }

    fn view(
        &self,
        tx: &Transmitter<GameListModel>,
        rx: &Receiver<GameListView>,
    ) -> ViewBuilder<HtmlElement> {
        let tx_fetch = tx.contra_filter_map(
            |result: &Result<Vec<String>, crate::api::FetchError>| match result {
                Ok(ids) => Some(GameListModel::ReplaceList {
                    game_ids: Rc::new(ids.clone()),
                }),
                Err(_) => None,
            },
        );
        tx_fetch.send_async(crate::api::get_game_list());
        let tx_cloned = tx.clone();
        let rx_patch = rx.branch_map(move |msg| Patch::Replace {
            index: 0,
            value: GameList::game_ul(&tx_cloned, &msg.game_ids),
        });
        builder! {
            <main class="game-list">
                <slot patch:children=rx_patch>
                    <ul />
                </slot>
            </main>
        }
    }
}

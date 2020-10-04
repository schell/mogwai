#![allow(unused_braces)]

mod model;
use model::*;

use mogwai::prelude::*;

use wasm_bindgen::prelude::*;

use log::*;
use std::panic;

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap_throw();

    let root = game();
    root.run()
}

fn game() -> View<HtmlElement> {
    view! {
        <div class="game">
           <div class="game-board">
               { Gizmo::from(Board::default()) }
           </div>
           <div class="game-info">
              <div> </div>
              <ol> </ol>
           </div>
        </div>
    }
}

#[derive(Clone)]
pub enum BoardAction {
    Click(u8),
}

#[derive(Clone)]
pub enum BoardUpdate {
    Mark(String),
}

impl Component for Board {
    type ModelMsg = BoardAction;
    type ViewMsg = BoardUpdate;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &BoardAction, tx_view: &Transmitter<BoardUpdate>, _sub: &Subscriber<BoardAction>) {
        unimplemented!()
    }

    fn view(&self, tx: &Transmitter<BoardAction>, rx: &Receiver<BoardUpdate>) -> ViewBuilder<HtmlElement> {
        let result = view! {
            <div>
                <div class="status"> "Next player: X" </div>
                <div class="board-row">
                    { Gizmo::from(Square { id: SquareID::new(0) }) }
                    { Gizmo::from(Square { id: SquareID::new(1) }) }
                    { Gizmo::from(Square { id: SquareID::new(2) }) }
                </div>
                <div class="board-row">
                    { Gizmo::from(Square { id: SquareID::new(3) }) }
                    { Gizmo::from(Square { id: SquareID::new(4) }) }
                    { Gizmo::from(Square { id: SquareID::new(5) }) }
                </div>
                <div class="board-row">
                    { Gizmo::from(Square { id: SquareID::new(6) }) }
                    { Gizmo::from(Square { id: SquareID::new(7) }) }
                    { Gizmo::from(Square { id: SquareID::new(8) }) }
                </div>
            </div>
        };
        result.into()
    }
}

struct Square {
    id: SquareID,
}

#[derive(Clone)]
enum SquareEvents {
    Click(SquareID)
}

#[derive(Clone)]
enum SquareUpdates {
    Draw(String)
}

impl Component for Square {
    type ModelMsg = SquareEvents;
    type ViewMsg = SquareUpdates;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &SquareEvents, tx_view: &Transmitter<SquareUpdates>, _sub: &Subscriber<SquareEvents>) {
        match *msg {
            SquareEvents::Click(sq) => {
                tx_view.send(&SquareUpdates::Draw("X".to_string()))
            }
        }
    }

    fn view(&self, tx: &Transmitter<SquareEvents>, rx: &Receiver<SquareUpdates>) -> ViewBuilder<HtmlElement> {
        let id = self.id;
        let result = view! {
            <button class="square" on:click=tx.contra_map(move |_| SquareEvents::Click(id))>
                {("",
                    rx.branch_map(|update| match update {
                        Updates::Draw(s) => String::from(s)
                    })
                )}
            </button>
        };
        result.into()
    }
}

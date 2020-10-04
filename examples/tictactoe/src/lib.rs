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
pub enum BoardEvent {
    Click(u8),
}

#[derive(Clone)]
pub enum BoardRefresh {
    Mark(String),
}

impl Component for Board {
    type ModelMsg = BoardEvent;
    type ViewMsg = BoardRefresh;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &BoardEvent, tx_view: &Transmitter<BoardRefresh>, _sub: &Subscriber<BoardEvent>) {
        unimplemented!()
    }

    fn view(&self, tx: &Transmitter<BoardEvent>, rx: &Receiver<BoardRefresh>) -> ViewBuilder<HtmlElement> {
        builder! {
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
        }
    }
}

struct Square {
    id: SquareID,
}

#[derive(Clone)]
enum SquareEvent {
    Click(SquareID)
}

#[derive(Clone)]
enum SquareRefresh {
    Draw(String)
}

impl Component for Square {
    type ModelMsg = SquareEvent;
    type ViewMsg = SquareRefresh;
    type DomNode = HtmlElement;

    fn update(&mut self, msg: &SquareEvent, tx_view: &Transmitter<SquareRefresh>, _sub: &Subscriber<SquareEvent>) {
        // match *msg {
        //     SquareEvent::Click(sq) => {
        //         tx_view.send(&SquareRefresh::Draw("X".to_string()))
        //     }
        // };
    }

    fn view(&self, tx: &Transmitter<SquareEvent>, rx: &Receiver<SquareRefresh>) -> ViewBuilder<HtmlElement> {
        let id = self.id;
        let result = view! {
            <button class="square" on:click=tx.contra_map(move |_| SquareEvent::Click(id))>
                {("",
                    rx.branch_map(|update| match update {
                        SquareRefresh::Draw(s) => String::from(s)
                    })
                )}
            </button>
        };
        result.into()
    }
}

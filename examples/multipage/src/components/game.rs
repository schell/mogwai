use mogwai::prelude::*;
use std::convert::TryInto;

pub enum Update {
    Cell {
        row: u16,
        column: u16,
        value: String,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct CellInteract {
    pub row: u16,
    pub column: u16,
    pub flag: bool,
}

fn panicky_u16(value: usize) -> u16 {
    value.try_into().unwrap()
}

#[allow(unused_braces)]
fn board_cell(
    coords: (u16, u16),
    tx: Transmitter<CellInteract>,
    rx: Receiver<Update>,
) -> ViewBuilder<HtmlElement> {
    let (col, row) = coords;
    let rx_text = rx.branch_filter_map(move |update| match update {
        Update::Cell {
            row: y,
            column: x,
            value,
        } if *x == col && *y == row => Some(value.to_owned()),
        _ => None,
    });
    builder! {
        <td
            valign="middle"
            align="center"
            style="height: 50px; width: 50px; border: 1px solid black;"
            on:click=tx.contra_map(move |event: &Event| CellInteract {
                row,
                column: col,
                // TODO: Check if `MouseEvent` has modifier keys held (alt, ctrl)
                flag: event.bubbles(),
            })
        >
            // Cells initialize to empty but may update if revealed or clicked
            {(" ", rx_text)}
        </td>
    }
}

fn board_row<'b>(
    row: u16,
    cells: Vec<&'b str>,
    tx: Transmitter<CellInteract>,
    rx: Receiver<Update>,
) -> ViewBuilder<HtmlElement> {
    let children = cells
        .into_iter()
        .enumerate()
        .map(|(col, _)| board_cell((panicky_u16(col), row), tx.clone(), rx.branch()));
    let mut tr = ViewBuilder {
        element: Some("tr".to_owned()),
        ..ViewBuilder::default()
    };
    for child in children {
        tr.with(child);
    }
    tr
}

#[allow(unused_braces)]
pub fn board<'a>(
    cells: Vec<Vec<&'a str>>,
    tx: Transmitter<CellInteract>,
    rx: Receiver<Update>,
) -> ViewBuilder<HtmlElement> {
    let children = cells
        .into_iter()
        .enumerate()
        .map(|(row, cells)| board_row(panicky_u16(row), cells, tx.clone(), rx.branch()));
    let mut tbody: ViewBuilder<HtmlElement> = ViewBuilder {
        element: Some("tbody".to_owned()),
        ..ViewBuilder::default()
    };
    for child in children {
        tbody.with(child);
    }
    builder! {
        <table>
            {tbody}
        </table>
    }
}

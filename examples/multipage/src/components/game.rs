use mogwai::prelude::*;
use web_sys::MouseEvent;

pub enum Update {
    Cell {
        row: usize,
        column: usize,
        value: String,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct CellInteract {
    pub row: usize,
    pub column: usize,
    pub flag: bool,
}


/// Create a `<td>` representing a single game cell. `coords` are expected to be `(column, row)` or
/// `(x, y)` in the grid. Interactions are transmitted to `tx` and new text to display is received
/// by `rx`.
#[allow(unused_braces)]
fn board_cell(
    coords: (usize, usize),
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

/// Create a `<tr>` representing a row of game cells. Interactions are transmitted to `tx` and new
/// text to display is received by `rx`.
fn board_row<'a>(
    row: usize,
    initial_cells: Vec<&'a str>,
    tx: Transmitter<CellInteract>,
    rx: Receiver<Update>,
) -> ViewBuilder<HtmlElement> {
    let children = initial_cells
        .into_iter()
        .enumerate()
        .map(|(col, _)| board_cell((col, row), tx.clone(), rx.branch()));
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
    initial_cells: Vec<Vec<&'a str>>,
    tx: Transmitter<CellInteract>,
    rx: Receiver<Update>,
) -> ViewBuilder<HtmlElement> {
    let children = initial_cells
        .into_iter()
        .enumerate()
        .map(|(row, cells)| board_row(row, cells, tx.clone(), rx.branch()));
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

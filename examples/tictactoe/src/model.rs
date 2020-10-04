#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SquareMark {
    Empty,
    X,
    O,
}

impl Default for SquareMark {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Debug)]
pub enum NextPlayerChange {
    Start,
    Next,
}

impl NextPlayerChange {
    fn mutate(&self, v: &SquareMark) -> SquareMark {
        use SquareMark::*;
        match self {
            Self::Start => X,
            Self::Next => match v {
                X => O,
                O => X,
                Empty => Empty,
            },
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SquareID(u8);

impl SquareID {
    pub fn new(id: u8) -> Self {
        SquareID(id)
    }
}

#[derive(Copy, Clone)]
pub struct Board {
    pub squares: [SquareMark; 9],
}

impl Default for Board {
    fn default() -> Self {
        let mut squares = [SquareMark::default(); 9];
        Self { squares }
    }
}

impl<'a> Board {
    pub fn calculate_winner(&self) -> bool {
        const LINES: [[usize; 3]; 8] = [
            [0, 1, 2],
            [3, 4, 5],
            [6, 7, 8],
            [0, 3, 6],
            [1, 4, 7],
            [2, 5, 8],
            [0, 4, 8],
            [2, 4, 6],
        ];
        // LINES
        //     .iter()
        //     .map(|win| {
        //         // what mark is at each position?
        //         win.iter()
        //             .map(|id| Square::get(self.squares[*id], &template).0.mark)
        //     })
        //     .any(|win| {
        //         // are any of the lines all the same mark?
        //         use SquareMark::Empty;
        //         win.fold_first(|a, b| match a {
        //             Empty => Empty,
        //             _ => {
        //                 if a == b {
        //                     a
        //                 } else {
        //                     Empty
        //                 }
        //             }
        //         })
        //             .unwrap()
        //             != Empty
        //     })
        false
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum Status {
    Playing,
    Won,
}

impl Default for Status {
    fn default() -> Self {
        Self::Playing
    }
}

#[derive(Debug)]
pub enum StatusChange {
    Won,
}

impl StatusChange {
    fn mutate(&self, _v: &Status) -> Status {
        match self {
            Self::Won => Status::Won,
        }
    }
}


use anyhow::{anyhow, Result};

use crate::parse::{idx_to_name};

pub const UNINIT: usize = -1i64 as usize;
pub const NEG_SIGN: &str = "----";
pub const POS_SIGN: &str = "++++";

// TODO: date and time types
#[derive(Debug,Clone,PartialEq)]
pub enum Arg {
    End,
    Op(String),
    // Plus, Minus, UPlus, UMinus, Divide, Multiply, Percent, Concat, // & Power,
    Eq(String),
    // Equal, NotEqual, Less, Greater, LessEqual, GreaterEqual,
    OBracket(String),
    // OBracket, OSqBracket,
    CBracket(String),
    // CBracket, CSqBracket,
    Str(String),
    Rng(Option<String>, Vec<Pos>), // TODO: support sheet_name in expressions
    Number(f64),
    Func(String, usize), // Name, number or arguments
    Bool(bool),
    Comma,
}

impl Arg {
    // pub fn is_num(&self) -> bool {
    //     if let Arg::Number(_) = self {
    //         return true;
    //     }
    //     false
    // }
    // pub fn is_str(&self) -> bool {
    //     if let Arg::Str(_) = self {
    //         return true;
    //     }
    //     false
    // }
    // pub fn is_range(&self) -> bool {
    //     if let Arg::Rng(_) = self {
    //         return true;
    //     }
    //     false
    // }
    pub fn is_func(&self) -> bool {
        if let Arg::Func(_, _) = self {
            return true;
        }
        false
    }
    // pub fn is_value(&self) -> bool {
    //     match self {
    //         Arg::Number(_) | Arg::Str(_) | Arg::Bool(_) | Arg::Rng(_) => true,
    //         _ => false,
    //     }
    // }
    pub fn title(&self) -> String {
        match self {
            Arg::End => String::new(),
            Arg::Op(s)| Arg::Eq(s)| Arg::OBracket(s)| Arg::CBracket(s)| Arg::Str(s) => s.to_string(),
            Arg::Rng(_, v) => if v.len() == 1 {
                let col_fixed = if v[0].fixed_col { "$" } else { "" };
                let row_fixed = if v[0].fixed_row { "$" } else { "" };
                format!("{}{}{}{}", col_fixed, idx_to_name(v[0].col), row_fixed, v[0].row+1)
            } else if v.len() == 2 {
                let col0_fixed = if v[0].fixed_col { "$" } else { "" };
                let row0_fixed = if v[0].fixed_row { "$" } else { "" };
                if v[0].full_col {
                    format!("{}{}:{}{}", col0_fixed, idx_to_name(v[0].col), col0_fixed, idx_to_name(v[0].col))
                } else if v[0].full_row {
                    format!("{}{}:{}{}", row0_fixed, v[0].row+1, row0_fixed, v[0].row+1)
                } else {
                    let col1_fixed = if v[1].fixed_col { "$" } else { "" };
                    let row1_fixed = if v[1].fixed_row { "$" } else { "" };
                    format!("{}{}{}{}:{}{}{}{}",
                        col0_fixed, idx_to_name(v[0].col), row0_fixed, v[0].row+1,
                        col1_fixed, idx_to_name(v[1].col), row1_fixed, v[1].row+1
                    )
                }
            } else {
                String::from("#VALUE!")
            },
            Arg::Number(f) => format!("{}", f), // TODO: format?
            Arg::Func(name, _) => name.to_string(),
            Arg::Bool(b) => if *b {String::from("TRUE") } else { String::from("FALSE") },
            Arg::Comma => String::from(","),
        }
    }
    // Like `title` but returns parsable string
    pub fn to_expr(&self) -> String {
        match self {
            Arg::Str(s) => format!("\"{}\"", s.replace("\"", "\"\"\"")),
            _ => self.title(),
        }
    }
    // TODO: remove?
    // pub fn in_range(&self, col: usize, row: usize) -> bool {
    //     match self {
    //         Arg::Rng(_, v) => if v.len() == 1 {
    //             v[0].col == col && v[0].row == row
    //         } else {
    //             v[0].col >= col && v[0].row >= row && v[1].col <= col && v[1].row <= row
    //         },
    //         _ => false,
    //     }
    // }
    pub fn move_by(&mut self, dcol: isize, drow: isize) {
        if dcol == 0 && drow == 0 {
            return;
        }
        match self {
            Arg::Rng(_, v) => {
                let mut vnew: Vec<Pos> = Vec::new();
                let mut changed = false;
                for mut pos in v.iter().cloned() {
                    if !pos.fixed_col && dcol != 0 {
                        let newcol = pos.col as isize + dcol;
                        pos.col = if newcol < 0 { 0 } else { newcol as usize};
                        changed = true;
                    }
                    if !pos.fixed_row && drow != 0 {
                        let newrow = pos.row as isize + drow;
                        pos.row = if newrow < 0 { 0 } else { newrow as usize};
                        changed = true;
                    }
                    vnew.push(pos);
                }
                if changed {
                    std::mem::swap(self, &mut Arg::Rng(None, vnew));
                }
            },
            _ => {},
        }
    }
    pub fn shift_range(&mut self, base_col: usize, base_row: usize, dcol: isize, drow: isize) {
        if dcol == 0 && drow == 0 {
            return;
        }
        match self {
            Arg::Rng(_, v) => {
                let mut vnew: Vec<Pos> = Vec::new();
                let mut changed = false;
                for mut pos in v.iter().cloned() {
                    if !pos.fixed_col && dcol != 0 && pos.col >= base_col {
                        let newcol = pos.col as isize + dcol;
                        pos.col = if newcol < 0 { 0 } else { newcol as usize};
                        changed = true;
                    }
                    if !pos.fixed_row && drow != 0 && pos.row >= base_row {
                        let newrow = pos.row as isize + drow;
                        pos.row = if newrow < 0 { 0 } else { newrow as usize};
                        changed = true;
                    }
                    vnew.push(pos);
                }
                if changed {
                    std::mem::swap(self, &mut Arg::Rng(None, vnew));
                }
            },
            _ => {},
        }
    }
}

#[derive(Debug,Copy,Clone, PartialEq)]
pub struct Pos {
    pub col: usize,
    pub row: usize,
    pub fixed_col: bool,
    pub fixed_row: bool,
    pub full_col: bool, // TODO: support in formulae
    pub full_row: bool, // TODO: support in formulae
}

impl Default for Pos {
    fn default() -> Pos {
        Pos {
            col: UNINIT, fixed_col: false, full_col: false,
            row: UNINIT, fixed_row: false, full_row: false,
        }
    }
}

impl Pos {
    pub fn new(col: usize, row: usize) -> Pos {
        Pos{col, row, ..Pos::default()}
    }
}

pub fn err_msg(errcode: u16) -> &'static str {
    match errcode {
        0 => "",
        2 => "#RECURSE",
        3 => "#DIV/0",
        _ => "#VALUE!",
    }
}

const COL_SHIFT: u64 = 100000; // max number of columns 18000+, take next 10th power
pub fn pos_to_id(col: usize, row: usize) -> u64 {
    row as u64 * COL_SHIFT + col as u64
}
pub fn id_to_pos(id: u64) -> (usize, usize) {
    let (col, row) = (id % COL_SHIFT, id / COL_SHIFT);
    (col as usize, row as usize)
}


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
    Rng(Vec<Pos>),
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
            Arg::Rng(v) => if v.len() == 1 { // TODO: fixed and full
                format!("{}{}", idx_to_name(v[0].col), v[0].row+1)
            } else if v.len() == 2 {
                format!("{}{}:{}{}", idx_to_name(v[0].col), v[0].row+1, idx_to_name(v[1].col), v[1].row+1)
            } else {
                String::from("#VALUE!")
            },
            Arg::Number(f) => format!("{}", f), // TODO: format?
            Arg::Func(name, _) => name.to_string(),
            Arg::Bool(b) => if *b {String::from("TRUE") } else { String::from("FALSE") },
            Arg::Comma => String::from(","),
        }
    }
}

#[derive(Debug,Copy,Clone, PartialEq)]
pub struct Pos {
    pub col: usize,
    pub row: usize,
    pub fixed_col: bool,
    pub fixed_row: bool,
    pub full_col: bool,
    pub full_row: bool,
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

pub fn cr_to_uid(col: usize, row: usize) -> usize {
    row * 100_000 + col
}

use std::char;
use std::f64;
use std::convert::TryFrom;
use std::fmt;

use anyhow::{anyhow, Result};

use crate::ops::{Pos,Arg, UNINIT};

const NUM_LETTERS: usize = 26;
const TWO_LETTERS: usize = NUM_LETTERS * NUM_LETTERS;
const THREE_LETTERS: usize = NUM_LETTERS * NUM_LETTERS * NUM_LETTERS;
pub const MAX_COLS: usize = NUM_LETTERS + TWO_LETTERS + THREE_LETTERS; // Three letter names only
pub const MAX_ROWS: usize = 99999; // TODO: support for more rows
pub const MAX_COL_NAME_LEN: usize = 3;
pub const DEF_NUM_WIDTH: u16 = 5; // TODO: autocolumn width for row numbers

#[derive(Debug,Copy,Clone)]
pub enum Range {
    Single(Pos), // no selection, only current cell is a range
    Multi(Pos, Pos), // something selected
    Row(usize), // entire row
    Col(usize), // entire col
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Range::Single(p) => write!(f, "{}{}", idx_to_name(p.col), p.row+1),
            Range::Multi(p1, p2) => write!(f, "{}{}:{}{}", idx_to_name(p1.col), p1.row+1, idx_to_name(p2.col), p2.row+1),
            Range::Col(c) => write!(f, "COL {}", idx_to_name(*c)),
            Range::Row(r) => write!(f, "ROW {}", *r),
        }
    }
}

impl Range {
    pub fn indices(&self) -> (usize, usize, usize, usize) {
        match self {
            Range::Single(c) => {
                (c.col, c.row, c.col, c.row)
            },
            Range::Multi(c1, c2) => {
                (c1.col, c1.row, c2.col, c2.row)
            },
            _ => unimplemented!(),
        }
    }
}

fn inc_char(inc: usize) -> char {
    char::from_u32(('A' as usize + inc) as u32).unwrap_or('?')
}
pub fn idx_to_name(id: usize) -> String {
    assert!(id < MAX_COLS); // TODO: return error?
    if id < NUM_LETTERS {
        return format!("{}", inc_char(id));
    }
    let id = id - NUM_LETTERS;
    if id < TWO_LETTERS {
        let first = id / NUM_LETTERS;
        let second = id % NUM_LETTERS;
        return format!("{}{}", inc_char(first), inc_char(second));
    }

    let id = id - TWO_LETTERS;
    let first = id / (NUM_LETTERS * NUM_LETTERS);
    let second = id % (NUM_LETTERS * NUM_LETTERS);
    let third = second % NUM_LETTERS;
    let second = second / NUM_LETTERS;
    return format!("{}{}{}", inc_char(first), inc_char(second), inc_char(third));
}
pub fn name_to_idx(name: &str) -> Result<usize> {
    let chrs: Vec<char> = name.chars().collect();
    if name.is_empty() || chrs.len() > MAX_COL_NAME_LEN {
        return Err(anyhow!("Name too long '{}'", name));
    }
    let mut id = 0usize;
    for c in &chrs {
        if !('A'..='Z').contains(&c) && !('a'..='z').contains(&c) {
            return Err(anyhow!("Characters must be in A..Z range: found '{}'", c));
        }
        id *= NUM_LETTERS;
        if ('A'..='Z').contains(&c) {
            id += ((*c as u32) - ('A' as u32)) as usize;
        } else {
            id += ((*c as u32) - ('a' as u32)) as usize;
        }
    }
    match chrs.len() {
        3 => id += TWO_LETTERS + NUM_LETTERS,
        2 => id += NUM_LETTERS,
        _ => {},
    }
    if id > MAX_COLS {
        return Err(anyhow!("Invalid column name '{}'", name));
    }
    Ok(id)
}

// ---------------------------------

pub fn is_white(c: char) -> bool {
    // c == ' ' || c == '\t' || c == '\n' || c == '\r'
    c.is_ascii_whitespace()
}

fn is_ident_start(c: char) -> bool {
    // ('a'..='z').contains(&c) || ('A'..='Z').contains(&c) ||
    // c == '_'
    c.is_alphabetic() || c == '_'
}

fn is_ident_cont(c: char) -> bool {
    // ('a'..='z').contains(&c) || ('A'..='Z').contains(&c) ||
    // ('0'..'9').contains(&c) ||
    // c == '_' || c == '.'
    c.is_alphabetic() || c.is_ascii_digit() || c == '_' || c == '.'
}

fn is_sheet_name(c: char) -> bool {
    c.is_alphabetic() || c.is_ascii_digit() ||
        c == '-' || c == '_' || c == ':' || c == '/' || c == '(' || c == ')' || c == '+'
}

pub fn skip_white(s: &str) -> &str {
    let mut start = s.len();
    for (cidx, c) in s.char_indices() {
        if !is_white(c) {
            start = cidx;
            break;
        }
    }
    &s[start..]
}

pub fn parse_comma(s: &str) -> (&str, bool) {
    parse_literal(s, ",")
}

pub fn parse_literal<'a, 'b>(s: &'a str, what: &'b str) -> (&'a str, bool) {
    if !s.starts_with(what) {
        return (s, false);
    }
    let st = &s[what.len()..];
    (st, true)
}

pub fn parse_any_literal<'a, 'b>(s: &'a str, what: &'b[&'b str]) -> (&'a str, String) {
    for w in what {
        if s.starts_with(w) {
            let st = &s[w.len()..];
            return (st, w.to_string());
        }
    }
    (s, String::new())
}

pub fn parse_any_char<'a, 'b>(s: &'a str, what: &'b str) -> (&'a str, String) {
    match s.chars().next() {
        None => (s, String::new()),
        Some(c) => {
            if what.contains(c) {
                let st = String::from(c);
                (&s[st.len()..], st)
            } else {
                (s, String::new())
            }
        },
    }
}

pub fn parse_while<F>(s: &str, f: F) -> (&str, String)
    where F: Fn(char) -> bool
{
    let mut res = String::new();
    for c in s.chars() {
        if !f(c) {
            break;
        }
        res.push(c);
    }
    let st = &s[res.len()..];
    (st, res)
}

pub fn parse_ident(s: &str) -> (&str, String) {
    let (st, mut ident) = match s.chars().next() {
        Some(c) if is_ident_start(c) => {
            let mut r = String::new();
            r.push(c);
            (&s[r.len()..], r)
        },
        _ => return (s, String::new()),
    };
    let (st, rest) = parse_while(st, |c| is_ident_cont(c));
    if !rest.is_empty() {
        (st, ident+&rest)
    } else {
        (st, ident)
    }
}

fn parse_coord(s: &str) -> Result<(&str, Pos)> {
    // info!("IN: {}", s);
    let mut c = Pos::default();
    // $?
    let (st, ok) = parse_literal(s, "$");
    if ok {
        c.fixed_col = true;
    }
    // AB??
    let (st, col_name) = parse_while(st, |c| (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z'));
    if col_name.is_empty() {
        c.full_row = true;
        c.fixed_row = c.fixed_col;
        c.fixed_col = false;
    } else {
        c.col = name_to_idx(&col_name)?;
    }
    // $?
    let (st, ok) = parse_literal(st, "$");
    if ok {
        c.fixed_row = true;
    }
    // 12??
    let (st, row_name) = parse_while(st, |c| c >= '0' && c <= '9' );
    if row_name.is_empty() {
        c.full_col = true;
    } else {
        let row = row_name.parse::<usize>()?;
        if row == 0 {
            return Err(anyhow!("Invalid row index: 0 ({})", row_name));
        }
        c.row = row - 1;
    }
    // sanity check
    if (row_name.is_empty() && col_name.is_empty()) || (c.col == UNINIT && c.row == UNINIT) {
        return Err(anyhow!("Invalid cell address {}: failed to parse col/row", s));
    }
    if (c.fixed_col && c.full_col) || (c.fixed_row && c.full_row) {
        return Err(anyhow!("Invalid cell address {}: full row/col cannot be fixed", s));
    }
    Ok((st, c))
}

// - Full rectangle format: $A$1:$c$8
// - Full column: A:A
// - Full row: 2:2
// - All '$' are optional
// - Full column and full row cannot contain '$'
// - Only characters: $,:,a-z,A-Z,0-1
// - ':' can be only one and in the middle
pub fn parse_range(s: &str) -> Result<(&str, Vec<Pos>)> {
    let mut coords: Vec<Pos> = Vec::new();
    // First coord
    let (st, mut c1) = parse_coord(s)?;
    // :?
    let (st, ok) = parse_literal(st, ":");
    if !ok {
        // if !st.is_empty() {
        //     return Err(anyhow!("Invalid cell address {}", s));
        // }
        coords.push(c1);
        return Ok((st, coords));
    }
    // Second coord?
    let mut c2 = Pos::default();
    let (st, mut c2) = parse_coord(st)?;
    // if !st.is_empty() {
    //     return Err(anyhow!("Invalid cell address {}", s));
    // }
    // Sort and test
    if c1.full_col && c1.col != UNINIT && (!c2.full_col || c2.col != c1.col) {
        return Err(anyhow!("Invalid cell address {}: full column names differ", s));
    }
    if c1.full_row && c1.row != UNINIT && (!c2.full_row || c2.row != c1.row) {
        return Err(anyhow!("Invalid cell address {}: full row indices differ", s));
    }
    if ((c1.row == UNINIT && c2.row != UNINIT) || (c1.row != UNINIT && c2.row == UNINIT)) && (c1.col != c2.col) {
        return Err(anyhow!("Invalid range address {}: half-opened column with different column names", s));
    }
    if ((c1.col == UNINIT && c2.col != UNINIT) || (c1.col != UNINIT && c2.col == UNINIT)) && (c1.row != c2.row) {
        return Err(anyhow!("Invalid range address {}: half-opened row with different row indices", s));
    }
    if c2.full_col && (c1.fixed_col ^ c2.fixed_col) {
        return Err(anyhow!("Cannot set fixed column for one side for a full column {}", s));
    }
    if c2.full_row && (c1.fixed_row ^ c2.fixed_row) {
        return Err(anyhow!("Cannot set fixed row for one side for a full row {}", s));
    }
    if c1.col > c2.col {
        std::mem::swap(&mut c1.col, &mut c2.col);
    }
    if c1.row > c2.row {
        std::mem::swap(&mut c1.row, &mut c2.row);
    }
    coords.push(c1);
    coords.push(c2);
    Ok((st, coords))
}

fn parse_sheet_name(s: &str) -> Result<(&str, String)> {
    let (st, ok) = parse_literal(s, "'");
    let (st, name) = if ok {
        let (st, name) = parse_while(&st, |c| c != '\'');
        let (st, ok) = parse_literal(st, "'");
        if !ok {
            return Err(anyhow!("Unclosed single quote: {}", s));
        }
        (st, name)
    } else {
        parse_while(st, |c| is_sheet_name(c))
    };
    let (st, ok) = parse_literal(st, "!");
    if ok {
        Ok((st, name))
    } else {
        Ok((s, String::new()))
    }
}

// TODO: support half-opened range H2:H = all items in H column starting from row 2
pub fn parse_full_range(s: &str) -> Result<(&str, Vec<Pos>, String)> {
    let (st, sheet) = parse_sheet_name(s)?;
    let (st, coords) = parse_range(st)?;
    Ok((st, coords, sheet))
}

pub fn parse_int(s: &str) -> Result<(&str, i64)> {
    let (st, num_str) = parse_while(s, |c| c.is_ascii_digit());
    if num_str.is_empty() {
        return Err(anyhow!("Invalid integer number: {}", s));
    }
    match i64::from_str_radix(&num_str, 10) {
        Err(e) => Err(anyhow!(e.to_string())),
        Ok(i) => Ok((st, i)),
    }
}

pub fn parse_float(s: &str) -> Result<(&str, f64)> {
    let mut float = String::new();
    let (st, base) = parse_while(s, |c| c.is_ascii_digit());
    if base.is_empty() {
        return Err(anyhow!("Invalid fload-point number: {}", s));
    }
    float += &base;
    let (st, ok) = parse_literal(st, ".");
    let st = if ok {
        let (stt, base) = parse_while(st, |c| c.is_ascii_digit());
        float += ".";
        float += &base;
        stt
    } else {
        st
    };
    let (st, ch) = parse_any_char(st, "eE");
    if ch.is_empty() {
        let f = float.parse::<f64>()?;
        if !f.is_finite() {
            return Err(anyhow!("Invalid floating point value: {}", s));
        }
        return Ok((st, f));
    }
    float += &ch;
    let (st, ch) = parse_any_char(st, "-+");
    if !ch.is_empty() {
        float += &ch;
    }
    let (st, base) = parse_while(st, |c| c.is_ascii_digit());
    if base.is_empty() {
        float += "0";
    } else {
        float += &base;
    }
    let f = float.parse::<f64>()?;
    if !f.is_finite() {
        return Err(anyhow!("Invalid floating point value: {}", s));
    }
    Ok((st, f))
}

pub fn parse_string(s: &str) -> Result<(&str, String)> {
    let (mut st, mut ok) = parse_literal(s, "\"");
    if !ok {
        return Err(anyhow!("No opening quote mark: '{}'", s));
    }
    let mut ss = String::new();
    loop {
        let (st_in, base) = parse_while(st, |c| c != '"');
        ss += &base;
        let (st_in, ok) = parse_literal(st_in, "\"\"");
        if ok {
            ss += "\"";
            st = st_in;
            continue;
        }
        let (st_in, ok) = parse_literal(st_in, "\"");
        if !ok {
            return Err(anyhow!("No closing quote mark found"));
        }
        st = st_in;
        break;
    }
    Ok((st, ss))
}

pub fn parse_func(s: &str) -> (&str, String) {
    let (st, id) = parse_ident(s);
    if id.is_empty() {
        return (s, id);
    }
    let (stl, ok) = parse_literal(st, "(");
    if ok {
        return (st, id);
    }
    (s, String::new())
}

// TODO: "10:10" is parsed as Float(10.0)
pub fn parse_arg(s: &str) -> Result<(&str, Arg)> {
    if s.is_empty() {
        return Ok((s, Arg::End));
    }
    let (st, ok) = parse_comma(s);
    if ok {
        return Ok((st, Arg::Comma))
    }
    let (st, ok) = parse_literal(s, "\"");
    if ok {
        let (st, val) = parse_string(s)?;
        return Ok((st, Arg::Str(val)));
    }
    let (st, fn_name) = parse_func(s);
    if !fn_name.is_empty() {
        return Ok((st, Arg::Func(fn_name, 0)));
    }
    if let Ok((st, f)) = parse_float(s) {
        return Ok((st, Arg::Number(f)));
    }
    let (st, id) = parse_while(s, |c| ('a'..='z').contains(&c) || ('A'..='Z').contains(&c) || c == '$');
    if !id.is_empty() {
        let (st, rng, sheet_name) = parse_full_range(s)?;
        if sheet_name.is_empty() {
            return Ok((st, Arg::Rng(None, rng)));
        } else {
            return Ok((st, Arg::Rng(Some(sheet_name), rng)));
        }
    }
    let (st, op) = parse_any_literal(s, &["<>", "<=", ">=", "<", ">", "="]);
    if !op.is_empty() {
        return Ok((st, Arg::Eq(op)));
    }
    let (st, op) = parse_any_literal(s, &["+", "-", "/", "*", "%", "&", "^"]);
    if !op.is_empty() {
        return Ok((st, Arg::Op(op)));
    }
    let (st, ob) = parse_any_literal(s, &["(", "["]);
    if !ob.is_empty() {
        return Ok((st, Arg::OBracket(ob)));
    }
    let (st, cb) = parse_any_literal(s, &[")", "]"]);
    if !cb.is_empty() {
        return Ok((st, Arg::CBracket(cb)));
    }
    Err(anyhow!("failed to parse: '{}'", s))
}

#[rustfmt::skip]
#[cfg(test)]
mod parse_test {
    use super::*;
    use crate::ops::*;
    #[test]
    fn coord_parse_ok() {
        struct Tst {
            val: &'static str,
            r1: Pos,
            r2: Pos,
            len: usize,
        }
        let tests: Vec<Tst> = vec![
            Tst {
                val: "A1", len: 1,
                r1: Pos{col: 0, row: 0, ..Pos::default() },
                r2: Pos::default(),
            },
            Tst {
                val: "$b2", len: 1,
                r1: Pos{col: 1, row: 1, fixed_col: true, ..Pos::default() },
                r2: Pos::default(),
            },
            Tst {
                val: "b$2", len: 1,
                r1: Pos{col: 1, row: 1, fixed_row: true, ..Pos::default() },
                r2: Pos::default(),
            },
            Tst {
                val: "Y3489", len: 1,
                r1: Pos{col: NUM_LETTERS-2, row: 3488, ..Pos::default() },
                r2: Pos::default(),
            },
            Tst {
                val: "$Y3489", len: 1,
                r1: Pos{col: NUM_LETTERS-2, row: 3488, fixed_col: true, ..Pos::default() },
                r2: Pos::default(),
            },
            Tst {
                val: "$Y$3489", len: 1,
                r1: Pos{col: NUM_LETTERS-2, row: 3488, fixed_col: true, fixed_row: true, ..Pos::default() },
                r2: Pos::default(),
            },
            Tst {
                val: "AB19", len: 1,
                r1: Pos{col: NUM_LETTERS+1, row: 18, ..Pos::default() },
                r2: Pos::default(),
            },
            Tst {
                val: "AA:AA", len: 2,
                r1: Pos{col: NUM_LETTERS, full_col: true, ..Pos::default() },
                r2: Pos{col: NUM_LETTERS, full_col: true, ..Pos::default() },
            },
            Tst {
                val: "C2:AB4", len: 2,
                r1: Pos{col: 2, row: 1, ..Pos::default() },
                r2: Pos{col: NUM_LETTERS+1, row: 3, ..Pos::default() },
            },
            Tst { // the same as above with wrong order of cols and rows - must be fixed automatically
                val: "C4:AB2", len: 2,
                r1: Pos{col: 2, row: 1, ..Pos::default() },
                r2: Pos{col: NUM_LETTERS+1, row: 3, ..Pos::default() },
            },
            Tst {
                val: "c2:Ab4", len: 2,
                r1: Pos{col: 2, row: 1, ..Pos::default() },
                r2: Pos{col: NUM_LETTERS+1, row: 3, ..Pos::default() },
            },
            Tst {
                val: "B2:B", len: 2,
                r1: Pos{col: 1, row: 1, ..Pos::default() },
                r2: Pos{col: 1, full_col: true, ..Pos::default() },
            },
        ];
        for t in tests {
            let (_s, r) = parse_range(t.val).unwrap();
            assert_eq!(r.len(), t.len);
            assert_eq!(r[0], t.r1);
            if t.len > 1 {
                assert_eq!(r[1], t.r2);
            }
        }
    }
    #[test]
    fn coord_parse_fail() {
        let v: Vec<&str> = vec![ ":A1", "", "ZXCD:89", "A:B", "1:D", "$A:A", "A:$A", "1:$1" ];
        for s in v {
            let r = parse_range(s);
            assert!(r.is_err(), "{} = {:?}", s, r);
        }
    }
    #[test]
    fn ident_parse_ok() {
        struct Tst {
            val: &'static str,
            id: String,
        }
        let tests: Vec<Tst> = vec![
            Tst{val: "123.4", id: String::new()},
            Tst{val: "(23)", id: String::new()},
            Tst{val: "zs(23)", id: String::from("zs")},
            Tst{val: "zкирs2-", id: String::from("zкирs2")},
            Tst{val: "func.name", id: String::from("func.name")},
            Tst{val: "ир", id: String::from("ир")},
            Tst{val: "zs_0(23)", id: String::from("zs_0")},
        ];
        for test in tests {
            let (_s, v) = parse_ident(test.val);
            assert_eq!(v, test.id, "{}", test.val);
        }
    }
    #[test]
    fn full_range_parse_ok() {
        struct Tst {
            val: &'static str,
            sheet: String,
            coords: Vec<Pos>,
        }
        let tests: Vec<Tst> = vec![
            Tst{
                val: "a1:b1",
                sheet: String::new(),
                coords: vec![Pos{col: 0, row: 0, ..Pos::default()},
                            Pos{col: 1, row: 0, ..Pos::default()},
                ],
            },
            Tst{
                val: "1ac!a1:b1",
                sheet: String::from("1ac"),
                coords: vec![Pos{col: 0, row: 0, ..Pos::default()},
                            Pos{col: 1, row: 0, ..Pos::default()},
                ],
            },
            Tst{
                val: "1/ac!c4+d5",
                sheet: String::from("1/ac"),
                coords: vec![Pos{col: 2, row: 3, ..Pos::default()}],
            },
            Tst{
                val: "sheet-name3!a2:b1",
                sheet: String::from("sheet-name3"),
                coords: vec![Pos{col: 0, row: 0, ..Pos::default()},
                            Pos{col: 1, row: 1, ..Pos::default()},
                ],
            },
            Tst{
                val: "'sheet one'!a2:b1",
                sheet: String::from("sheet one"),
                coords: vec![Pos{col: 0, row: 0, ..Pos::default()},
                            Pos{col: 1, row: 1, ..Pos::default()},
                ],
            },
        ];
        for test in tests {
            let (_s, coords, sheet) = parse_full_range(test.val).unwrap();
            assert_eq!(sheet, test.sheet, "{}", test.val);
            assert_eq!(coords.len(), test.coords.len(), "{}", test.val);
            assert_eq!(coords[0], test.coords[0], "{}", test.val);
            if coords.len() > 1 {
                assert_eq!(coords[1], test.coords[1], "{}", test.val);
            }
        }
    }
    #[test]
    fn to_name() {
        let v: Vec<usize> = vec![
            NUM_LETTERS-1, NUM_LETTERS, NUM_LETTERS+1,
            TWO_LETTERS+NUM_LETTERS-1, TWO_LETTERS+NUM_LETTERS, TWO_LETTERS+NUM_LETTERS+1,
            MAX_COLS-2, MAX_COLS-1,
        ];
        let r: Vec<&str> = vec!["Z", "AA", "AB", "ZZ", "AAA", "AAB", "ZZY", "ZZZ"];
        for (idx, val) in v.iter().enumerate() {
            let res = idx_to_name(*val);
            assert_eq!(res, r[idx], "{}", val);
        }
    }
    #[test]
    fn to_idx() {
        let v: Vec<&str> = vec!["H", "Z", "AA", "AB", "CD", "YC", "ZZ", "AAA", "AAB", "DEF", "UAC", "ZZY", "ZZZ"];
        for val in v {
            let idx = name_to_idx(val).unwrap();
            let back = idx_to_name(idx);
            assert_eq!(val, back, "{}={}", val, idx);
        }
    }
    #[test]
    fn parse_int_test() {
        struct Tst {
            st: &'static str,
            vl: i64,
            err: bool,
        }
        let tests: Vec<Tst> = vec![
            Tst{st: "a", vl: 0, err: true},
            Tst{st: "8888888888888888888888888888888888", vl: 0, err: true},
            Tst{st: "239", vl: 239, err: false},
            Tst{st: "8839.2", vl: 8839, err: false},
        ];
        for test in tests {
            let r = parse_int(test.st);
            if test.err {
                if let Ok((_s, ii)) = r {
                    info!("{} == {}", test.st, ii);
                }
                assert!(r.is_err(), "{}", test.st);
            } else {
                let (_s, val) = r.unwrap();
                assert_eq!(val, test.vl, "{}", test.st);
            }
        }
    }
    #[test]
    fn parse_float_test() {
        struct Tst {
            st: &'static str,
            vl: f64,
            err: bool,
        }
        let tests: Vec<Tst> = vec![
            Tst{st: "a", vl: 0.0, err: true},
            Tst{st: "5.0e4278", vl: 1e-2, err: true},

            Tst{st: "1.0e-4278", vl: 0.0, err: false},
            Tst{st: "239+2", vl: 239.0, err: false},
            Tst{st: "239", vl: 239.0, err: false},
            Tst{st: "239e4", vl: 2390000.0, err: false},
            Tst{st: "8839.2", vl: 8839.2, err: false},
            Tst{st: "1.0e2", vl: 100.0, err: false},
            Tst{st: "86.", vl: 86.0, err: false},
            Tst{st: "239.+2", vl: 239.0, err: false},
            Tst{st: "239.e+vb", vl: 239.0, err: false},
            Tst{st: "86.e7", vl: 86e7, err: false},
            Tst{st: "11.0e+2", vl: 1100.0, err: false},
            Tst{st: "11.0e+2.3", vl: 1100.0, err: false},
            Tst{st: "1.0e-02", vl: 1e-2, err: false},
        ];
        for test in tests {
            let r = parse_float(test.st);
            if test.err {
                if let Ok((_s, v)) = r {
                    info!("[{}] --> [{}]", test.st, v);
                }
                assert!(r.is_err(), "{}", test.st);
            } else {
                let (_s, val) = r.unwrap();
                assert_eq!(val, test.vl, "{}", test.st);
            }
        }
    }
    #[test]
    fn parse_string_test() {
        struct Tst {
            st: &'static str,
            rs: String,
            err: bool,
        }
        let tests: Vec<Tst> = vec![
            Tst{st: "abc", rs: String::new(), err: true},
            Tst{st: "\"abc", rs: String::new(), err: true},
            Tst{st: "\"abc\"\"", rs: String::from("abc"), err: true},

            Tst{st: "\"abc\"\"\"", rs: String::from("abc\""), err: false},
            Tst{st: "\"deabc\"zz", rs: String::from("deabc"), err: false},
            Tst{st: "\"deabc\"\"zz\"", rs: String::from("deabc\"zz"), err: false},
            Tst{st: "\"deabc\"\"zz\"\"ui\"", rs: String::from("deabc\"zz\"ui"), err: false},
        ];
        for test in tests {
            let r = parse_string(test.st);
            if test.err {
                if let Ok((_s, ref v)) = r {
                    info!("[{}] --> [{}]", test.st, v);
                }
                assert!(r.is_err(), "{}", test.st);
            } else {
                let (_s, val) = r.unwrap();
                assert_eq!(val, test.rs, "{}", test.st);
            }
        }
    }
    #[test]
    fn parse_func_test() {
        struct Tst {
            st: &'static str,
            rs: String,
        }
        let tests: Vec<Tst> = vec![
            Tst{st: "abc", rs: String::new()},
            Tst{st: "1abc(", rs: String::new()},
            Tst{st: "abc+(1, 2)", rs: String::from("")},

            Tst{st: "ab360c(A2, B1)", rs: String::from("ab360c")},
            Tst{st: "abc.func(A2, B1)", rs: String::from("abc.func")},
            Tst{st: "_abc(A2, B1)", rs: String::from("_abc")},
        ];
        for test in tests {
            let (_s, val) = parse_func(test.st);
            assert_eq!(val, test.rs, "{}", test.st);
        }
    }
    #[test]
    fn parse_arg_test() {
        struct Tst {
            st: &'static str,
            rs: Arg,
            err: bool,
        }
        let tests: Vec<Tst> = vec![
            Tst{st: "\"abc", rs: Arg::Number(0.0), err: true},
            Tst{st: "@abc", rs: Arg::Number(0.0), err: true},

            Tst{st: "jf", rs: Arg::Rng(vec![Pos{full_col: true, col: 265, ..Pos::default()}]), err: false},
            Tst{st: "$b2", rs: Arg::Rng(vec![Pos{fixed_col: true,col:1,row:1, ..Pos::default()}]), err: false},
            Tst{st: "abc.tr(jf)", rs: Arg::Func(String::from("abc.tr"), 0), err: false},
            Tst{st: "+abc.tr(jf)", rs: Arg::Op(String::from("+")), err: false},
            Tst{st: "<>abc.tr(jf)", rs: Arg::Eq(String::from("<>")), err: false},
            Tst{st: "(jf)", rs: Arg::OBracket(String::from("(")), err: false},
            Tst{st: "3.6e5abc.tr(jf)", rs: Arg::Number(3.6e5), err: false},
            Tst{st: "\"3.6e5\"\"abc.t\"r(jf)", rs: Arg::Str("3.6e5\"abc.t".to_string()), err: false},
            Tst{st: ",(jf)", rs: Arg::Comma, err: false},
        ];
        for test in tests {
            let r = parse_arg(test.st);
            if test.err {
                if let Ok((_s, ref val)) = r {
                    info!("Must be ERROR [{}]: {:?}", test.st, val);
                }
                assert!(r.is_err());
            } else {
                if r.is_err() {
                    info!("Must be OK: [{}]: {:?}", test.st, r);
                }
                let (_s, val) = r.unwrap();
                assert_eq!(val, test.rs, "{:?}", test.st);
            }
        }
    }
    #[test]
    fn parse_multi_arg_test() {
        struct Tst {
            st: &'static str,
            args: Vec<Arg>,
        }
        let tests: Vec<Tst> = vec![
            Tst{
                st: " fn.a(a1:b2,4) + a1*4.5%&\"ope\"",
                args: vec![
                    Arg::Func(String::from("fn.a"), 0),
                    Arg::OBracket(String::from("(")),
                    Arg::Rng(vec![
                        Pos{col:0,row:0,..Pos::default()},
                        Pos{col:1,row:1,..Pos::default()},
                    ]),
                    Arg::Comma,
                    Arg::Number(4.0),
                    Arg::CBracket(String::from(")")),
                    Arg::Op(String::from("+")),
                    Arg::Rng(vec![Pos{col:0,row:0,..Pos::default()}]),
                    Arg::Op(String::from("*")),
                    Arg::Number(4.5),
                    Arg::Op(String::from("%")),
                    Arg::Op(String::from("&")),
                    Arg::Str(String::from("ope")),
                ],
            },
        ];
        for test in tests {
            let mut s = skip_white(test.st);
            for a in test.args {
                let r = parse_arg(s);
                let (st, val) = r.unwrap();
                assert_eq!(val, a, "{:?}", test.st);
                s = skip_white(st);
            }
        }
    }
    #[test]
    fn range_title() {
        let tests: Vec<&'static str> = vec![
            "C:C", "A1:D3", /*"10:10",*/ "$A3:G$7", "$H$6:$I$9", "GH345:XZU98100",
        ];
        for test in tests {
            let r = parse_arg(test);
            let (_st, val) = r.unwrap();
            let back = val.title();
            assert_eq!(back.as_str(), test, "{:?}", val);
        }
    }
    #[test]
    fn range_move() {
        struct Tst {
            base: &'static str,
            res: &'static str,
            dcol: isize,
            drow: isize,
        }
        let tests: Vec<Tst> = vec![
            Tst{ base: "A2:$B4", dcol: 2, drow: 3, res: "C5:$B7" },
            Tst{ base: "B$2:Z45", dcol: 3, drow: 2, res: "E$2:AC47" },
            Tst{ base: "C:C", dcol: 2, drow: 3, res: "E:E" },
            Tst{ base: "C:C", dcol: -1, drow: -2, res: "B:B" },
            Tst{ base: "BBB1000", dcol: 2, drow: 3, res: "BBD1003" },
            Tst{ base: "A2:$B4", dcol: -2, drow: -3, res: "A1:$B1" },
        ];
        for test in tests {
            let r = parse_arg(test.base);
            let (_st, mut val) = r.unwrap();
            val.move_by(test.dcol, test.drow);
            let title = val.title();
            assert_eq!(title.as_str(), test.res, "{:?}", val);
        }
    }
    #[test]
    fn range_shift() {
        struct Tst {
            st: &'static str,
            res: &'static str,
            dcol: isize,
            drow: isize,
            brow: usize,
            bcol: usize,
        }
        let tests: Vec<Tst> = vec![
            Tst{ st: "A2:E9", bcol: 10, brow: 10, dcol: 2, drow: 3, res: "A2:E9" },
            Tst{ st: "A2:$E9", bcol: 4, brow: 5, dcol: 0, drow: 0, res: "A2:$E9" },
            Tst{ st: "A2:$E9", bcol: 4, brow: 5, dcol: 2, drow: 0, res: "A2:$E9" },
            Tst{ st: "A2:E9", bcol: 4, brow: 5, dcol: 0, drow: 0, res: "A2:E9" },
            Tst{ st: "A2:E9", bcol: 4, brow: 5, dcol: 2, drow: 2, res: "A2:G11" },
            Tst{ st: "A2:E9", bcol: 4, brow: 5, dcol: -2, drow: -2, res: "A2:C7" },
            Tst{ st: "F12:I19", bcol: 4, brow: 5, dcol: 2, drow: 1, res: "H13:K20" },
        ];
        for test in tests {
            let r = parse_arg(test.st);
            let (_st, mut val) = r.unwrap();
            val.shift_range(test.bcol, test.brow, test.dcol, test.drow);
            let title = val.title();
            assert_eq!(title.as_str(), test.res, "{:?}", val);
        }
    }
}

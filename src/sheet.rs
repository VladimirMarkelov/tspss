use std::char;
use std::io::{Write,Read};
use std::fs::File;
use std::collections::HashMap;
use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use crossterm::{ style::{ Color} };
use crossterm::event::{KeyCode, Event, KeyModifiers};
use unicode_width::UnicodeWidthStr;
use bincode::{serialize_into, deserialize_from};

use crate::primitive::Screen;
use crate::ui::{Widget,Context,Transition,NOTHING};
use crate::edit::Edit;
use crate::strs;
use crate::parse::{idx_to_name, MAX_COLS, MAX_ROWS, DEF_NUM_WIDTH, Range, parse_float, parse_while, parse_arg,is_white};
use crate::ops::{Arg,Pos, err_msg, pos_to_id, id_to_pos};
use crate::stack::{str_expr_to_vec, expr_to_stack};
use crate::expr::{Expr};

const MIN_COL_WIDTH: u16 = 5;
const MAX_COL_WIDTH: u16 = 100; // TODO:
const DEF_COL_WIDTH: u16 = 10;
const END_OF_CELLS: usize = 9_999_888_777;
const NO_VALUE: u8 = 0xFF;
const CLR_8: u8 = 0x00;
const CLR_ANSI: u8 = 0x01;
const CLR_RGB: u8 = 0x02;
pub const VERSION: u16 = 1;

#[derive(Debug,Copy,Clone)]
pub enum CalcMode {
    Move,
    Edit,
    Select,
    Command,
    TempSelect,
    TempSelectStart,
}
#[derive(Copy,Clone,Debug)]
pub enum Align {
    Left,
    Right,
    Center,
}
pub struct Attr {
    pub fg: Color,
    pub bg: Color,
    pub align: Align,
    //format: ...,
    //readonly: ... ?
}
#[derive(Clone,Debug)]
pub struct OptionAttr {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub align: Option<Align>,
    //format: ...,
    //readonly: ... ?
}
impl Default for OptionAttr {
    fn default() -> OptionAttr {
        OptionAttr { fg: None, bg: None, align: None, }
    }
}
impl OptionAttr {
    fn is_default(&self) -> bool {
        self.fg.is_none() && self.bg.is_none() && self.align.is_none()
    }
}

#[derive(Debug,Copy,Clone)]
pub enum SelectType {
    V,
    Shift,
}

#[derive(Debug,Copy,Clone)]
enum MoveBy {
    Cell(usize), // count
    Page(usize), // count
    Edge,
    End,
}

/*
 * enum Action { // for undo
 * CellText(Pos, TextOld, TextNew),
 * CellFg(Pos, FgOld, FgNew),
 * CellBg(Pos, BgOld, BgNew),
 * Filter(Vec<usize>),
 * HideRows(Vec<usize>),
 * ShowRows(Vec<usize>),
 * HideCols(Vec<usize>),
 * ShowCols(Vec<usize>),
 * Resize(col#, oldWidth, newWidth),
 * }
*/

fn col8_to_u8(clr: Color) -> Result<u8> {
    let c = match clr {
        Color::Black => 0u8,
        Color::Red => 1u8,
        Color::DarkRed => 2u8,
        Color::Green => 3u8,
        Color::DarkGreen => 4u8,
        Color::Yellow => 5u8,
        Color::DarkYellow => 6u8,
        Color::Blue => 7u8,
        Color::DarkBlue => 8u8,
        Color::Magenta => 9u8,
        Color::DarkMagenta => 10u8,
        Color::Cyan => 11u8,
        Color::DarkCyan => 12u8,
        Color::Grey => 13u8,
        Color::DarkGrey => 14u8,
        Color::White => 15u8,
        _ => return Err(anyhow!("invalid color")),
    };
    Ok(c)
}

fn u8_to_col8(clr: u8) -> Result<Color> {
    let c = match clr {
        0u8 => Color::Black,
        1u8 => Color::Red,
        2u8 => Color::DarkRed,
        3u8 => Color::Green,
        4u8 => Color::DarkGreen,
        5u8 => Color::Yellow,
        6u8 => Color::DarkYellow,
        7u8 => Color::Blue,
        8u8 => Color::DarkBlue,
        9u8 => Color::Magenta,
        10u8 => Color::DarkMagenta,
        11u8 => Color::Cyan,
        12u8 => Color::DarkCyan,
        13u8 => Color::Grey,
        14u8 => Color::DarkGrey,
        15u8 => Color::White,
        _ => return Err(anyhow!("invalid color index")),
    };
    Ok(c)
}

fn save_color<W:Write+Copy>(f: W, clr: &Option<Color>) -> Result<()> {
    let c = if let Some(cl) = clr {
        cl
    } else {
        serialize_into(f, &NO_VALUE)?;
        return Ok(());
    };
    match c {
        Color::AnsiValue(val) => {
            serialize_into(f, &CLR_ANSI)?;
            serialize_into(f, &val)?;
        },
        Color::Rgb{ r, g, b } => {
            serialize_into(f, &CLR_RGB)?;
            serialize_into(f, &r)?;
            serialize_into(f, &g)?;
            serialize_into(f, &b)?;
        },
        _ => {
            let idx = col8_to_u8(*c)?;
            serialize_into(f, &CLR_8)?;
            serialize_into(f, &idx)?;
        },
    }
    Ok(())
}
fn save_align<W:Write+Copy>(f: W, align: &Option<Align>) -> Result<()> {
    match align {
        None => serialize_into(f, &NO_VALUE)?,
        Some(a) => match a {
            Align::Left => serialize_into(f, &0u8)?,
            Align::Right => serialize_into(f, &1u8)?,
            Align::Center => serialize_into(f, &2u8)?,
        }
    }
    Ok(())
}

fn load_color<R:Read+Copy>(f: R) -> Result<Option<Color>> {
    let tp: u8 = deserialize_from(f)?;
    match tp {
        NO_VALUE => Ok(None),
        CLR_ANSI => {
            let a: u8 = deserialize_from(f)?;
            Ok(Some(Color::AnsiValue(a)))
        },
        CLR_RGB => {
            let r: u8 = deserialize_from(f)?;
            let g: u8 = deserialize_from(f)?;
            let b: u8 = deserialize_from(f)?;
            Ok(Some(Color::Rgb{r, g, b}))
        },
        CLR_8 => {
            let a: u8 = deserialize_from(f)?;
            let c = u8_to_col8(a)?;
            Ok(Some(c))
        }
        _ => Err(anyhow!("invalid color type {}", tp)),
    }
}
fn load_align<R:Read+Copy>(f: R) -> Result<Option<Align>> {
    let tp: u8 = deserialize_from(f)?;
    match tp {
        NO_VALUE => Ok(None),
        0u8 => Ok(Some(Align::Left)),
        1u8 => Ok(Some(Align::Right)),
        2u8 => Ok(Some(Align::Center)),
        _ => Err(anyhow!("invalid align index {}", tp)),
    }
}

#[derive(Clone,Debug)]
pub struct Cell {
    pub val: String, // user input
    pub calculated: Arg, // calculated user input
    pub attr: OptionAttr,
    pub err: u16,
}
impl Default for Cell {
    fn default() -> Cell {
        Cell { val: String::new(), calculated: Arg::End, attr: Default::default(), err: 0, }
    }
}

impl Cell {
    pub fn is_expr(&self) -> bool { self.val.starts_with('=') }
    pub fn is_number(&self) -> bool {
        if let Arg::Number(_) = self.calculated { true } else { false }
    }
    pub fn align(&self) -> Align {
        match self.attr.align {
            Some(a) => a,
            None => if self.is_number() {
                Align::Right
            } else {
                Align::Left
            },
        }
    }
    pub fn title(&self) -> String {
        if self.err != 0 {
            return err_msg(self.err).to_string();
        }
        self.calculated.title()
    }
    fn is_default(&self) -> bool {
        self.val.is_empty() && self.attr.is_default()
    }
    pub fn save<W: Write+Copy>(&self, f: W) -> Result<()>{
        serialize_into(f, &self.val)?;
        save_align(f, &self.attr.align)?;
        save_color(f, &self.attr.fg)?;
        save_color(f, &self.attr.bg)?;
        // TODO: save format
        // TODO: save conditional color
        // TODO: save state(readonly/etc)
        Ok(())
    }
    fn load<R:Read+Copy>(f: R, version: u16) -> Result<Cell> {
        if version != VERSION {
            return Err(anyhow!("unsupported version {}", version)); // TODO:
        }
        let mut cell = Cell::default();
        cell.val = deserialize_from(f)?;
        info!("cell: {}", cell.val);
        cell.attr.align = load_align(f)?;
        info!("cell: {:?}", cell.attr.align);
        cell.attr.fg = load_color(f)?;
        cell.attr.bg = load_color(f)?;
        // TODO: load format
        // TODO: load conditional color
        // TODO: load state(readonly/etc)
        Ok(cell)
    }
}

struct SubRange {
    rng: Range,
    values: BTreeMap<u64, Cell>,
}

pub struct Sheet {
    pub name: String,
    pub first_row: usize, // top row in the screen
    pub first_col: usize, // most-left column in the screen
    pub cursor: Pos,
    pub fixed_rows: usize,
    pub fixed_cols: usize,
    select_start: Option<Pos>,
    select_end: Option<Pos>,
    select_type: SelectType,
    widths: HashMap<usize, u16>, // columns widths: colID <=> width
    // col_attrs: Vec<OptionAttr>, // column default attrs // see TODO: below
    // row_attrs: Vec<OptionAttr>, // row default attrs // TODO: when setting a new default, replace
    //                        // cell attrs with row ones for existing row/cols
    pub mode: CalcMode,
    pub dirty: bool,
    pub h: u16, // height and width of sheet cell area
    pub w: u16,
    pub cells: BTreeMap<u64, Cell>,
    pub max_row: usize, // maximum used column number
    pub max_col: usize, // maximum used row number
    yanked: Option<SubRange>,
}

impl Sheet {
    pub fn new(idx: usize, w: u16, h: u16) -> Sheet {
        Sheet {
            name: format!("page{}", idx+1),
            first_col: 0,
            first_row: 0,
            cursor: Pos::new(0, 0),
            select_start: None,
            select_end: None,
            select_type: SelectType::V,
            mode: CalcMode::Move,
            widths: HashMap::new(),
            fixed_cols: 0,
            fixed_rows: 0,
            dirty: false,
            w, h,
            cells: BTreeMap::new(),
            max_row: 0,
            max_col: 0,
            yanked: None,
        }
    }
    pub fn col_width(&self, col: usize) -> u16 {
        match self.widths.get(&col) {
            None => DEF_COL_WIDTH,
            Some(w) => *w,
        }
    }
    pub fn last_visible_col(&self) -> (usize, bool) {
        let mut colpos = DEF_NUM_WIDTH; // TODO: support more than 10000 rows
        if self.is_col_fixed() {
            for c in 0..self.fixed_cols {
                let cwidth = self.col_width(c);
                colpos += cwidth;
                if colpos >= self.w {
                    return (c, colpos == self.w);
                }
            }
        }
        for c in self.first_col..MAX_COLS {
            let cwidth = self.col_width(c);
            colpos += cwidth;
            if colpos >= self.w {
                return (c, colpos == self.w);
            }
            if c == MAX_COLS-1 {
                return (MAX_COLS-1, true);
            }
        }
        return (self.first_col, false)
    }
    pub fn last_visible_row(&self) -> usize {
        let h = self.h;
        let mut row = self.first_row;
        let mut h = h - 1; // Minus column headers
        while h != 0 {
            h -= 1;
            row += 1;
        }
        row
    }
    // fn row_to_screen(&self, row: usize) -> Option<u16> {
    //     let last = self..last_visible_row();
    //     if row < self.first_row || row > last {
    //         return None;
    //     }
    //     let mut pos = self.row + 1; // Plus column header
    //     for c in self.first_row..=last {
    //         if c == row {
    //             break;
    //         }
    //         pos += 1;
    //     }
    //     Some(pos)
    // }
    pub fn ensure_visible_col(&mut self) {
        let w = self.w;
        let col = self.cursor.col;
        let has_fixed = self.is_col_fixed();
        if col == self.first_col || (has_fixed && col < self.fixed_cols) {
            // info!("04. {} -- always visible", col);
            return;
        }
        if col < self.first_col && (!has_fixed || col >= self.fixed_cols) {
            // info!("01. {} -- {} -- {}", col, self.first_col, has_fixed);
            self.first_col = col;
            return;
        }
        let (last, full) = self.last_visible_col();
        if col < last || (col == last && full) {
            // info!("04. {} -- {} -- {}", col, last, full);
            return;
        }
        let cwidth = self.col_width(col);
        let mut filled = cwidth + self.fixed_col_width() as u16 + DEF_NUM_WIDTH; // TODO: row id width
        if filled >= w {
            // info!("02. {} -- {} -- {} -- {}", col, self.first_col, has_fixed, filled);
            self.first_col = col;
            return;
        }
        for cid in (0..col).rev() {
            let cwidth = self.col_width(cid);
            filled += cwidth;
            if filled > w {
                // info!("03. {} -- {} -- {} -- {} = {}", col, self.first_col, has_fixed, filled, cid);
                self.first_col = cid + 1;
                return;
            }
        }
    }
    pub fn ensure_visible_row(&mut self) {
        let h = self.h;
        let row = self.cursor.row;
        let h = h - 1; // Ignore header line
        let first_visible = self.first_row + self.fixed_rows;
        if row >= first_visible && row < self.last_visible_row() {
            return;
        }
        if row >= self.last_visible_row() {
            self.first_row = row - h as usize + 1;
            return;
        }

        if self.fixed_rows == 0 || self.fixed_rows >= h as usize {
            self.first_row = row;
            return;
        }
        if row < self.fixed_rows {
            return;
        }
        self.first_row = row - self.fixed_rows;
    }
    pub fn cell(&self, col: usize, row: usize) -> Cell {
        let id = pos_to_id(col, row);
        match self.cells.get(&id) {
            None => Default::default(),
            Some(v) => v.clone(),
        }
    }
    fn parse_value(&self, text: &str) -> Arg {
        if text.is_empty() {
            return Arg::End;
        }
        let caps = text.to_uppercase();
        if caps.as_str() == "TRUE" {
            return Arg::Bool(true);
        } else if caps.as_str() == "FALSE" {
            return Arg::Bool(false);
        }
        match parse_float(text) {
            Err(_) => Arg::Str(text.to_string()),
            Ok((rest, val)) => if rest.is_empty() {
                Arg::Number(val)
            } else {
                Arg::Str(text.to_string())
            }
        }
    }
    fn calc_expr(&mut self, expr: &str, uid: u64) -> Result<Arg> {
        let args = str_expr_to_vec(expr)?;
        let args = expr_to_stack(&args)?;
        let mut expr = Expr::default(); // TODO: must be a member of Sheet
        expr.cache.insert(uid, 1);
        expr.calculate(&args, self)
    }
    pub fn set_cell_calc_value(&mut self, col: usize, row: usize, val: Result<Arg>) {
        let id = pos_to_id(col, row);
        if let Some(cell) = self.cells.get_mut(&id) {
            match val {
                Ok(v) => cell.calculated = v,
                Err(e) => {
                    info!("{:?}", e);
                    cell.err = 1;
                    cell.calculated = Arg::End;
                },
            }
        }
    }
    pub fn set_cell_text(&mut self, col: usize, row: usize, text: &str) {
        let id = pos_to_id(col, row);
        let text = text.trim();
        {
            let cell = self.cell(col, row);
            if cell.val == text {
                return;
            }
        }
        let mut do_calc = false;
        do_calc = text.starts_with('=');
        if let Some(cell) = self.cells.get_mut(&id) {
            cell.val = text.to_string();
            cell.err = 0;
        } else {
            let mut cell = Cell::default();
            cell.val = text.to_string();
            self.cells.insert(id, cell);
        }
        if !do_calc {
            let v = self.parse_value(text);
            self.set_cell_calc_value(col, row, Ok(v));
            self.recalc_cells();
            self.dirty = true;
            return;
        }
        info!("calculate {}", text);
        let val = self.calc_expr(&text[1..], id);
        self.set_cell_calc_value(col, row, val);
        self.recalc_cells();
        self.dirty = true;
    }
    pub fn set_cell_attr(&mut self, col: usize, row: usize, attr: OptionAttr) {
        let id = pos_to_id(col, row);
        self.dirty = true;
        if let Some(cell) = self.cells.get_mut(&id) {
            if attr.fg.is_some() {
                cell.attr.fg = attr.fg;
            }
            if attr.bg.is_some() {
                cell.attr.bg = attr.bg;
            }
            if attr.align.is_some() {
                cell.attr.align = attr.align;
            }
            return;
        }

        let mut cell = Cell::default();
        if attr.fg.is_some() {
            cell.attr.fg = attr.fg;
        }
        if attr.bg.is_some() {
            cell.attr.bg = attr.bg;
        }
        if attr.align.is_some() {
            cell.attr.align = attr.align;
        }
        self.set_cell(col, row, cell);
    }
    fn set_cell(&mut self, col: usize, row: usize, cell: Cell) {
        if col > self.max_col {
            self.max_col = col;
        }
        if row > self.max_row {
            self.max_row = row;
        }
        let id = pos_to_id(col, row);
        info!("insert cell at {}x{} = {}", col, row, id);
        self.cells.insert(id, cell);
        self.dirty = true;
    }
    pub fn is_under_cursor(&self, col: usize, row: usize) -> bool {
        col == self.cursor.col && self.cursor.row == row
    }
    pub fn is_in_selection(&self, col: usize, row: usize) -> bool {
        if !self.is_in_select_mode() {
            return false;
        }
        match self.selected_range() {
            Range::Single(p) => {p.col == col && p.row == row},
            Range::Multi(p1, p2) => {col >= p1.col && col <= p2.col && row >= p1.row && row <= p2.row},
            Range::Col(c) => col == c,
            Range::Row(r) => row == r,
        }
    }
    // Priority (from highest):
    // - Selection attrs
    // - Cell attrs
    // - Column attrs
    // - Row attrs
    // - Default attrs
    pub fn cell_attr(&self, col: usize, row: usize) -> Attr {
        let selected = self.is_under_cursor(col, row);
        let in_selection = self.is_in_selection(col, row);
        let cell = self.cell(col, row);
        let col_attr = OptionAttr::default(); // TODO: look for
        let row_attr = OptionAttr::default(); // TODO: look for
        let fg = if selected {
            Color::Black
        } else if in_selection {
            Color::DarkBlue
        } else if let Some(f) = cell.attr.fg {
            f
        } else if let Some(f) = col_attr.fg {
            f
        } else if let Some(f) = row_attr.fg {
            f
        } else {
            Color::White // TODO: get base grid font color
        };
        let bg = if selected {
            Color::White
        } else if in_selection {
            Color::Blue
        } else if let Some(b) = cell.attr.bg {
            b
        } else if let Some(b) = col_attr.bg {
            b
        } else if let Some(b) = row_attr.bg {
            b
        } else {
            // TODO: get base grid back color
            if col % 2 == 0 { Color::Black } else { Color::DarkGrey }
        };
        let align = if let Some(a) = cell.attr.align {
            a
        } else if let Some(a) = col_attr.align {
            a
        } else if let Some(a) = row_attr.align {
            a
        } else {
            Align::Left
        };

        Attr { bg, fg, align, }
    }
    // TODO: use 'cnt'
    fn move_left(&mut self, _cnt: MoveBy) {
        // let has_fixed = self.is_col_fixed();
        // let first_visible = if has_fixed { self.fixed_cols } else { 0 }; // TODO:
        // if first_visible == self.cursor.col || (has_fixed && self.cursor.col < self.fixed_cols) {
        //     return;
        // }
        if self.cursor.col == 0 {
            return;
        }
        self.cursor.col -= 1; // TODO:
        self.ensure_visible_col();
    }
    // TODO: use 'cnt'
    fn move_right(&mut self, _cnt: MoveBy) {
        let last_visible = MAX_COLS - 1; // TODO:
        if self.cursor.col == last_visible {
            return;
        }
        self.cursor.col += 1; // TODO:
        self.ensure_visible_col();
    }
    // TODO: use 'cnt'
    fn move_up(&mut self, _cnt: MoveBy) {
        if self.cursor.row == 0 {
            return;
        }
        if self.fixed_rows == 0 || self.fixed_row_height() as u16 >= self.h-1 || self.cursor.row > self.first_row+self.fixed_rows {
            self.cursor.row -= 1; // TODO:
            self.ensure_visible_row();
            return;
        }
        if self.first_row > 0 {
            self.first_row -= 1;
        }
        self.cursor.row -= 1;
        self.ensure_visible_row();
    }
    // TODO: use 'cnt'
    fn move_down(&mut self, _cnt: MoveBy) {
        let last_visible = MAX_ROWS - 1; // TODO:
        if self.cursor.row == last_visible {
            return;
        }
        self.cursor.row += 1; // TODO:
        self.ensure_visible_row();
    }
    pub fn arrow_left(&mut self, md: KeyModifiers) -> Transition {
        match md {
            KeyModifiers::NONE => {
                if self.select_start.is_some() && self.select_end.is_none() && !self.is_select_v() {
                    self.cancel_select();
                }
                self.move_left(MoveBy::Cell(1));
                Transition::None
            },
            KeyModifiers::SHIFT => {
                if self.select_start.is_none() || self.select_end.is_some() || self.is_select_v() {
                    self.start_select(SelectType::Shift);
                }
                self.move_left(MoveBy::Cell(1));
                Transition::None // TODO: selection change
            },
            /*
            (KeyModifiers::SHIFT|KeyModifiers::CONTROL)=>  {
                Transition::None // TODO: selection change by page
            },
            KeyModifiers::CONTROL => {
                Transition::None // TODO: move by page
            },
            */
            _ => Transition::EventPass,
        }
    }
    pub fn arrow_right(&mut self, md: KeyModifiers)-> Transition  {
        match md {
            KeyModifiers::NONE => {
                if self.select_start.is_some() && self.select_end.is_none() && !self.is_select_v() {
                    self.cancel_select();
                }
                self.move_right(MoveBy::Cell(1));
                Transition::None
            },
            KeyModifiers::SHIFT => {
                if self.select_start.is_none() || self.select_end.is_some() || self.is_select_v() {
                    self.start_select(SelectType::Shift);
                }
                self.move_right(MoveBy::Cell(1));
                Transition::None // TODO: selection change
            },
            /*
            (KeyModifiers::SHIFT|KeyModifiers::CONTROL)=>  {
                Transition::None // TODO: selection change by page
            },
            KeyModifiers::CONTROL => {
                Transition::None // TODO: move by page
            },
            */
            _ => Transition::EventPass,
        }
    }
    pub fn arrow_down(&mut self, md: KeyModifiers)-> Transition  {
        match md {
            KeyModifiers::NONE => {
                if self.select_start.is_some() && self.select_end.is_none() && !self.is_select_v() {
                    self.cancel_select();
                }
                self.move_down(MoveBy::Cell(1));
                Transition::None
            },
            KeyModifiers::SHIFT => {
                if self.select_start.is_none() || self.select_end.is_some() || self.is_select_v() {
                    self.start_select(SelectType::Shift);
                }
                self.move_down(MoveBy::Cell(1));
                Transition::None // TODO: selection change
            },
            /*
            (KeyModifiers::SHIFT|KeyModifiers::CONTROL)=>  {
                Transition::None // TODO: selection change by page
            },
            KeyModifiers::CONTROL => {
                Transition::None // TODO: move by page
            },
            */
            _ => Transition::EventPass,
        }
    }
    pub fn arrow_up(&mut self, md: KeyModifiers)-> Transition  {
        match md {
            KeyModifiers::NONE => {
                if self.select_start.is_some() && self.select_end.is_none() && !self.is_select_v() {
                    self.cancel_select();
                }
                self.move_up(MoveBy::Cell(1));
                Transition::None
            },
            KeyModifiers::SHIFT => {
                if self.select_start.is_none() || self.select_end.is_some() || self.is_select_v() {
                    self.start_select(SelectType::Shift);
                }
                self.move_up(MoveBy::Cell(1));
                Transition::None // TODO: selection change
            },
            /*
            (KeyModifiers::SHIFT|KeyModifiers::CONTROL)=>  {
                Transition::None // TODO: selection change by page
            },
            KeyModifiers::CONTROL => {
                Transition::None // TODO: move by page
            },
            */
            _ => Transition::EventPass,
        }
    }
    pub fn go_home(&mut self, md: KeyModifiers)-> Transition  {
        match md {
            KeyModifiers::NONE => {
                if self.is_col_fixed() {
                    if self.cursor.col <= self.fixed_cols {
                        self.cursor.col = 0;
                    } else {
                        self.cursor.col = self.fixed_cols;
                    }
                } else {
                    self.cursor.col = 0;
                }
                self.ensure_visible_col();
                Transition::None
            },
            _ => Transition::EventPass,
        }
    }
    pub fn go_end(&mut self, md: KeyModifiers)-> Transition  {
        match md {
            KeyModifiers::NONE => {
                self.cursor.col = self.max_col;
                self.ensure_visible_col();
                Transition::None
            },
            _ => Transition::EventPass,
        }
    }
    pub fn page_down(&mut self, md: KeyModifiers)-> Transition  {
        if md != KeyModifiers::NONE {
            return Transition::EventPass;
        }
        let has_fixed = self.is_row_fixed();
        let shift: usize = if has_fixed { self.h as usize - 1 - self.fixed_rows } else { self.h as usize - 1};
        if has_fixed && self.cursor.row < self.fixed_rows {
            self.cursor.row = self.fixed_rows;
        } else if self.cursor.row >= MAX_ROWS-1 {
            return Transition::None;
        } else if self.cursor.row + shift >= MAX_ROWS -1 {
            self.cursor.row = MAX_ROWS - 1;
        } else {
            self.cursor.row += shift;
        }
        self.ensure_visible_row();
        Transition::None
    }
    pub fn page_up(&mut self, md: KeyModifiers)-> Transition  {
        if md != KeyModifiers::NONE {
            return Transition::EventPass;
        }
        let has_fixed = self.is_row_fixed();
        let shift: usize = if has_fixed { self.h as usize - 1 - self.fixed_rows } else { self.h as usize - 1};
        if has_fixed && self.cursor.row > self.fixed_rows && self.fixed_rows+shift > self.cursor.row {
            self.cursor.row = self.fixed_rows;
        } else if self.cursor.row <= shift {
            self.cursor.row = 0;
        } else {
            self.cursor.row -= shift;
        }
        self.ensure_visible_row();
        Transition::None
    }

    pub fn clear_range(&mut self) {
        match self.selected_range() {
            Range::Single(pos) => self.set_cell_text(pos.col, pos.row, ""),
            Range::Multi(p1, p2) => {
                for r in p1.row..=p2.row {
                    for c in p1.col..=p2.col {
                        self.set_cell_text(c, r, "");
                    }
                }
            }
            Range::Col(_) => {}, // TODO:
            Range::Row(_) => {}, // TODO:
        }
        self.cancel_select();
    }

    pub fn is_in_select_mode(&self) -> bool {
        self.select_start.is_some()
    }
    pub fn is_select_v(&self) -> bool {
        match self.select_type {
            SelectType::V => true,
            _ => false,
        }
    }
    pub fn start_select(&mut self, tp: SelectType) {
        self.mode = if let CalcMode::Move = self.mode { CalcMode::Select } else { CalcMode::TempSelectStart };
        self.select_start = Some(Pos::new(self.cursor.col, self.cursor.row));
        self.select_end = None;
        self.select_type = tp;
    }
    pub fn finish_select(&mut self) {
        self.select_end = Some(Pos::new(self.cursor.col, self.cursor.row));
    }
    pub fn cancel_select(&mut self) {
        self.mode = if let CalcMode::TempSelectStart = self.mode { CalcMode::Edit } else { CalcMode::Move };
        self.select_start = None;
        self.select_end = None;
    }
    pub fn selected_range(&self) -> Range {
        if self.select_start.is_none() {
            return Range::Single(Pos::new(self.cursor.col, self.cursor.row));
        }
        let st = self.select_start.unwrap_or(Pos::new(0, 0));
        let (mut pos1, mut pos2) = if let Some(end) = self.select_end {
            (st, end)
        } else {
            (st, Pos::new(self.cursor.col, self.cursor.row))
        };
        if pos1.col > pos2.col {
            std::mem::swap(&mut pos1.col, &mut pos2.col);
        }
        if pos1.row > pos2.row {
            std::mem::swap(&mut pos1.row, &mut pos2.row);
        }
        Range::Multi(pos1, pos2)
    }
    pub fn save<W: Write+Copy>(&self, f: W) -> Result<()> {
        serialize_into(f, &self.name)?;
        serialize_into(f, &self.first_col)?;
        serialize_into(f, &self.first_row)?;
        serialize_into(f, &self.cursor.col)?;
        serialize_into(f, &self.cursor.row)?;
        serialize_into(f, &self.fixed_cols)?;
        serialize_into(f, &self.fixed_rows)?;
        // column attrs (first: number of items; N of column width)
        serialize_into(f, &self.widths.len())?; // TODO: skip cols with default width?
        for (idx, w) in &self.widths {
            serialize_into(f, idx)?;
            serialize_into(f, w)?;
        }
        // hidden rows(first: number of rows; N of row IDs) - not support, maybe far future
        serialize_into(f, &0usize)?; // TODO:
        // hidden cols(first: number of cols; N of col IDs) - not support, maybe far future
        serialize_into(f, &0usize)?; // TODO:
        // marked ranges (first: number of items; N of {char: mark, col+row+width+height})
        serialize_into(f, &0usize)?; // TODO:

        // cells
        for (id, cell) in self.cells.iter() {
            let (col, row) = id_to_pos(*id);
            serialize_into(f, &col)?;
            serialize_into(f, &row)?;
            cell.save(f)?;
        }
        // Mark the end of the sheet
        serialize_into(f, &END_OF_CELLS)?; // TODO:
        serialize_into(f, &END_OF_CELLS)?; // TODO:

        Ok(())
    }
    // TODO: pass here and to all 'load's version number
    pub fn load<R: Read+Copy>(f: R, width: u16, height: u16, version: u16) -> Result<Sheet> {
        if version != VERSION {
            return Err(anyhow!("unsupported version {}", version)); // TODO:
        }
        let mut sheet = Sheet::new(0, width, height);

        sheet.name = deserialize_from(f)?;
        sheet.first_col = deserialize_from(f)?;
        sheet.first_row = deserialize_from(f)?;
        sheet.cursor.col = deserialize_from(f)?;
        sheet.cursor.row = deserialize_from(f)?;
        sheet.fixed_cols = deserialize_from(f)?; // TODO: complain if size of fixed rows/cols bigger than screen
        sheet.fixed_rows = deserialize_from(f)?;
        // column attrs (first: number of items; N of column width)
        let col_attrs: usize = deserialize_from(f)?; // TODO:
        for _i in 0..col_attrs {
            let idx: usize = deserialize_from(f)?;
            let w: u16 = deserialize_from(f)?;
            sheet.widths.insert(idx, w);
        }
        // hidden rows(first: number of rows; N of row IDs) - not support, maybe far future
        let _hidden_rows: usize = deserialize_from(f)?; // TODO:
        // hidden cols(first: number of cols; N of col IDs) - not support, maybe far future
        let _hidden_cols: usize = deserialize_from(f)?; // TODO:
        // marked ranges (first: number of items; N of {char: mark, col+row+width+height})
        let _ranges: usize = deserialize_from(f)?; // TODO:

        // cells
        sheet.max_col = 0;
        sheet.max_row = 0;
        loop {
            let col: usize = deserialize_from(f)?;
            let row: usize = deserialize_from(f)?;
            info!("loading cell {}x{}", col, row);
            if col == END_OF_CELLS && row == END_OF_CELLS {
                break;
            }
            if col >= MAX_COLS {
                return Err(anyhow!("invalid column index: {}", col));
            }
            if row >= MAX_ROWS {
                return Err(anyhow!("invalid row index: {}", row));
            }
            let cell = Cell::load(f, version)?;
            let vv = cell.val.clone();
            sheet.set_cell(col, row, cell);
            sheet.set_cell_text(col, row, &vv);
            if col > sheet.max_col {
                sheet.max_col = col;
            }
            if row > sheet.max_row {
                sheet.max_row = row;
            }
        }
        sheet.recalc_cells();

        Ok(sheet)
    }
    // TODO: optimize
    fn recalc_cells(&mut self) {
        let mut hm: HashMap<(usize, usize, u64), String> = HashMap::new();
        for (id, cell) in self.cells.iter() {
            if !cell.is_expr() {
                continue;
            }
            let (col, row) = id_to_pos(*id);
            hm.insert((col, row, *id), cell.val[1..].to_string());
        }
        for ((col, row, uid), expr) in hm.drain() {
            let val = self.calc_expr(&expr, uid);
            self.set_cell_calc_value(col, row, val);
        }
    }
    pub fn resize_col(&mut self, col: usize, delta: i16) {
        info!("change col {} by {}", col, delta);
        let curr = (self.col_width(col) as i16 + delta) as u16;
        if curr < MIN_COL_WIDTH || curr > MAX_COL_WIDTH {
            return;
        }
        self.widths.insert(col, curr);
        self.dirty = true;
    }
    pub fn autosize_col(&mut self, col: usize) {
        let mut mx: u16 = 0;
        for row in 0..=self.max_row {
            let id = pos_to_id(col, row);
            if let Some(cell) = self.cells.get(&id) {
                let w = cell.title().width() as u16;
                if w > mx {
                    mx = w;
                }
                if mx > MAX_COL_WIDTH {
                    mx = MAX_COL_WIDTH;
                    break;
                }
            }
        }
        if mx < MIN_COL_WIDTH {
            mx = MIN_COL_WIDTH;
        }
        let w_old = self.col_width(col);
        self.dirty = w_old != mx;
        self.widths.insert(col, mx);
    }
    pub fn fixed_row_height(&self) -> usize {
        self.fixed_rows // TODO: use u16?
    }
    pub fn fix_row(&mut self, row: usize) -> Result<()> {
        if row > 1000 {
            return Err(anyhow!("Number of fixed rows is too big"));
        }
        self.fixed_rows = row;
        if self.fixed_row_height() >= self.h.into() {
            Err(anyhow!("Too many fixed rows"))
        } else {
            self.ensure_visible_row();
            Ok(())
        }
    }
    pub fn unfix_row(&mut self) {
        self.fixed_rows = 0;
    }
    pub fn fixed_col_width(&self) -> usize {
        if self.fixed_cols == 0 {
            return 0;
        }
        let mut w = 0;
        for col in 0..self.fixed_cols {
            w += self.col_width(col);
        }
        w.into() // TODO: use u16?
    }
    pub fn fix_col(&mut self, col: usize) -> Result<()> {
        if col > 200 {
            return Err(anyhow!("Number of fixed cols is too big"));
        }
        self.fixed_cols = col;
        if self.fixed_col_width() >= self.w.into() {
            Err(anyhow!("Too many fixed cols"))
        } else {
            self.first_col = col;
            self.ensure_visible_col();
            Ok(())
        }
    }
    pub fn unfix_col(&mut self) {
        self.fixed_cols = 0;
    }
    pub fn is_col_fixed(&self) -> bool {
        self.fixed_cols != 0 && (self.fixed_col_width() as u16) < self.w - DEF_NUM_WIDTH - 1
    }
    pub fn is_row_fixed(&self) -> bool {
        self.fixed_rows != 0 && ((self.fixed_row_height() as u16) < self.h-1)
    }
    pub fn yank(&mut self) {
        let rng = self.selected_range();
        let (col_start, row_start, col_end, row_end) = rng.indices();
        info!("YANK: {}x{} -  {}x{}", col_start, row_start, col_end, row_end);
        let mut values: BTreeMap<u64, Cell> = BTreeMap::new();
        for row in row_start..=row_end {
            for col in col_start..=col_end {
                let id = pos_to_id(col, row);
                if let Some(cell) = self.cells.get(&id) {
                    values.insert(id, cell.clone());
                }
            }
        }
        self.yanked = Some(SubRange{rng, values});
    }
    pub fn paste_yanked(&mut self) {
        match &self.yanked {
            None => return,
            Some(sub) => {
                let (col_start, row_start, col_end, row_end) = sub.rng.indices();
                if col_start == self.cursor.col && row_start == self.cursor.row {
                    return;
                }
                info!("PASTE: {}x{} -  {}x{}", col_start, row_start, col_end, row_end);
                let dcol = self.cursor.col as isize - col_start as isize;
                let drow = self.cursor.row as isize - row_start as isize;
                for row in row_start..=row_end {
                    for col in col_start..=col_end {
                        let id = pos_to_id(col, row);
                        let new_id = pos_to_id(col-col_start+self.cursor.col, row-row_start+self.cursor.row);
                        if let Some(cell) = sub.values.get(&id) {
                            let mut clone = cell.clone();
                            if !clone.is_expr() {
                                self.cells.insert(new_id, clone);
                            } else {
                                let expr = self.move_expression(&cell.val, dcol, drow);
                                info!("updated expr {} : {}", expr, cell.val);
                                clone.val = expr;
                                clone.calculated = Arg::End;
                                self.cells.insert(new_id, clone);
                            }
                        } else {
                            self.cells.remove(&new_id);
                        }
                    }
                }
                self.recalc_cells();
                self.dirty = true;
            }
        }
    }
    fn move_expression(&self, expr: &str, dcol: isize, drow: isize) -> String {
        let mut ex: &str = &expr["=".len()..];
        let mut output: String = String::from("=");
        loop {
            if ex.is_empty() {
                break;
            }
            let (st, spaces) = parse_while(ex, |c| is_white(c));
            if !spaces.is_empty() {
                output += &spaces;
            }
            match parse_arg(st) {
                Err(e) => return expr.to_string(),
                Ok((s, mut a)) => {
                    ex = s;
                    if let Arg::End = a {
                        break;
                    }
                    a.move_by(dcol, drow);
                    output += &a.to_expr();
                },
            }
        }
        output
    }
    /*
    fn undo() {
    }
    fn redo() {
    }
    fn copy() {
    }
    fn paste() {
    }
    */
}

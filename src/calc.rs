use std::char;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Write};

use anyhow::{anyhow, Result};
use crossterm::{ style::{ Color} };
use crossterm::event::{KeyCode, Event, KeyModifiers};
use unicode_width::UnicodeWidthStr;
use bincode::{serialize_into, deserialize_from};

use crate::primitive::Screen;
use crate::ui::{Widget,Context,Transition,NOTHING};
use crate::edit::Edit;
use crate::strs;
use crate::sheet::{Sheet, CalcMode, VERSION};
use crate::parse::{Range, idx_to_name, MAX_COLS, MAX_ROWS, DEF_NUM_WIDTH};
use crate::ops::{err_msg};

const MAIN_WIDGET: &str = "calc";

pub struct Calc {
    name: String,
    col: u16,
    row: u16,
    w: u16,
    h: u16,
    gen: usize,
    sheets: Vec<Sheet>,
    sheet: usize,
    ed_top: Edit,
    ed_bottom: Edit,
    err: Option<String>,
    /*
     * attr: Attr, // default attrs for even cols
     * alt_attr: Attr, // default attrs for odd cols
     * keys: ..., // collect pressed key to make a VIM-like command
    */
}

impl Default for Calc {
    fn default() -> Calc {
        let ctx = Context::new(0, 0);
        Calc {name: MAIN_WIDGET.to_string(), col: 0, row: 0, w: 0, h: 0, gen: 0,
            sheets: Vec::new(), sheet: 0, err: None,
            ed_top: Edit::new(&ctx, "ed-top", 1, 0, 0, Color::Black, Color::Grey, "[TOP]"),
            ed_bottom: Edit::new(&ctx, "ed-btm", 1, 0, 0, Color::Black, Color::Grey, "[BTM]"),
        }
    }
}

impl Calc {
    pub fn new(ctx: &Context) -> Calc {
        let def_sheet = Sheet::new(0, ctx.w, ctx.h-1);
        Calc {name: MAIN_WIDGET.to_string(), col: 0, row: 0, w: ctx.w, h: ctx.h-1, gen: 0,
            sheets: vec![def_sheet], sheet: 0, err: None,
            ed_top: Edit::new(ctx, "ed-top", 1, ctx.h-1, ctx.w-2, Color::Black, Color::Grey, "[TOP]"),
            ed_bottom: Edit::new(ctx, "ed-btm", 1, ctx.h-1, ctx.w-2, Color::Black, Color::Grey, "[BTM]"),
        }
    }

    fn draw_header(&self, ctx: &Context, scr: &mut Screen) -> Result<()> {
        let sheet = &self.sheets[self.sheet];
        // Column header
        let row_num_w = DEF_NUM_WIDTH; // TODO: support more than 10000 rows
        let has_fixed_cols = sheet.is_col_fixed();
        let mut pos = row_num_w;
        if has_fixed_cols {
            for i in 0..sheet.fixed_cols {
                let cwidth = sheet.col_width(i);
                if i % 2 == 0 {
                    scr.colors(Color::Blue, Color::Black); // TODO:
                } else {
                    scr.colors(Color::Blue, Color::DarkGrey);
                }
                let title = strs::center(&idx_to_name(i), cwidth.into());
                scr.write_string(&title, pos, self.row);
                pos += cwidth;
            }
        }
        for i in sheet.first_col..MAX_COLS {
            let cwidth = sheet.col_width(i);
            // info!("draw {}: {}", i, cwidth);
            if i % 2 == 0 {
                scr.colors(Color::White, Color::Black); // TODO:
            } else {
                scr.colors(Color::White, Color::DarkGrey);
            }
            let title = strs::center(&idx_to_name(i), cwidth.into());
            scr.write_string(&title, pos, self.row);
            pos += cwidth;
            if pos >= self.w {
                break;
            }
        }
        // Row header
        let mut pos = self.row+1;
        let has_fixed_rows = sheet.is_row_fixed();
        let from = if has_fixed_rows { sheet.first_row+sheet.fixed_rows } else { sheet.first_row };
        for i in sheet.first_row..MAX_ROWS+1 {
            let n = if has_fixed_rows && i < from {
                scr.colors(Color::Black, Color::White);
                i - sheet.first_row + 1
            } else {
                scr.colors(Color::White, Color::Black);
                i+1
            };
            let title = &format!("{:>width$}", n, width = row_num_w as usize);
            scr.write_string(&title, 0, pos);
            pos += 1;
            if pos > self.h {
                break;
            }
        }
        Ok(())
    }
    fn show_info(&self, ctx: &Context, scr: &mut Screen) -> Result<()> {
        if let Some(msg) = &self.err {
            scr.colors(Color::Red, Color::Black); // TODO:
            let w = msg.width();
            let title = msg.to_string() + &" ".repeat(self.w as usize - w);
            let title = strs::cut(&title, 0, (ctx.w - 1) as usize);
            scr.write_string(&title, 0, ctx.h - 1);
            return Ok(());
        }
        // TODO: top line with sheet name and current cell content or selected area
        let sheet = &self.sheets[self.sheet];
        let (col, row) = (sheet.cursor.col, sheet.cursor.row);
        let cell = sheet.cell(col, row);
        let addr = format!("{}", sheet.selected_range());
        let title = format!("[{}][{}]{}", sheet.name, addr, cell.val);
        let w = title.width();
        let title = title + &" ".repeat(self.w as usize - w);
        scr.colors(Color::White, Color::Black); // TODO:
        scr.write_string(&title, 0, ctx.h - 1);
        Ok(())
    }
    fn draw_mode(&self, ctx: &Context, scr: &mut Screen) -> Result<()> {
        scr.colors(Color::White, Color::Black); // TODO:
        match self.sheets[self.sheet].mode {
            CalcMode::Edit => scr.write_string("=", 0, ctx.h -1),
            CalcMode::Command => scr.write_string(":", 0, ctx.h -1),
            CalcMode::TempSelect | CalcMode::TempSelectStart => scr.write_string("@", 0, ctx.h - 1),
            _ => {},
        }
        Ok(())
    }
    // TODO: merge duplicated code
    fn draw_cells(&self, ctx: &Context, scr: &mut Screen) -> Result<()> {
        // TODO: double pass: first, draw background; second, draw text for non-empty cells
        let sheet = &self.sheets[self.sheet];
        let mut rowpos = self.row+1;
        let has_fixed_row = sheet.is_row_fixed();
        let has_fixed_col = sheet.is_col_fixed();
        let from = if has_fixed_row { sheet.first_row+sheet.fixed_rows } else { sheet.first_row };
        let row_num_w = DEF_NUM_WIDTH; // TODO: support more than 10000 rows
        if has_fixed_row {
            for r in 0..sheet.fixed_rows {
                let mut colpos = row_num_w;
                if has_fixed_col {
                    for c in 0..sheet.fixed_cols {
                        let cwidth = sheet.col_width(c);
                        let attr = sheet.cell_attr(c, r);
                        let cell = sheet.cell(c, r);
                        scr.colors(attr.fg, attr.bg);
                        // TODO: align
                        let mut title = cell.title();
                        if !title.is_empty() {
                            title = strs::cut(&title, 0, cwidth.into());
                        }
                        let l = title.width();
                        if l < cwidth.into() {
                            title = title + &" ".repeat(cwidth as usize - l);
                        }
                        scr.write_string(&title, colpos, rowpos);
                        colpos += cwidth;
                    }
                }
                for c in sheet.first_col..MAX_COLS {
                    let cwidth = sheet.col_width(c);
                    let attr = sheet.cell_attr(c, r);
                    let cell = sheet.cell(c, r);
                    scr.colors(attr.fg, attr.bg);
                    // TODO: align
                    let mut title = cell.title();
                    if !title.is_empty() {
                        title = strs::cut(&title, 0, cwidth.into());
                    }
                    let l = title.width();
                    if l < cwidth.into() {
                        title = title + &" ".repeat(cwidth as usize - l);
                    }
                    scr.write_string(&title, colpos, rowpos);
                    colpos += cwidth;
                    if colpos >= self.w {
                        break;
                    }
                }
                rowpos += 1;
                if rowpos >= self.h {
                    break;
                }
            }
        }
        for r in from..MAX_ROWS+1 {
            let mut colpos = row_num_w;
            if has_fixed_col {
                for c in 0..sheet.fixed_cols {
                    let cwidth = sheet.col_width(c);
                    let attr = sheet.cell_attr(c, r);
                    let cell = sheet.cell(c, r);
                    scr.colors(attr.fg, attr.bg);
                    // TODO: align
                    let mut title = cell.title();
                    if !title.is_empty() {
                        title = strs::cut(&title, 0, cwidth.into());
                    }
                    let l = title.width();
                    if l < cwidth.into() {
                        title = title + &" ".repeat(cwidth as usize - l);
                    }
                    scr.write_string(&title, colpos, rowpos);
                    colpos += cwidth;
                }
            }
            for c in sheet.first_col..MAX_COLS {
                let cwidth = sheet.col_width(c);
                let attr = sheet.cell_attr(c, r);
                let cell = sheet.cell(c, r);
                scr.colors(attr.fg, attr.bg);
                // TODO: align
                let mut title = cell.title();
                if !title.is_empty() {
                    title = strs::cut(&title, 0, cwidth.into());
                }
                let l = title.width();
                if l < cwidth.into() {
                    title = title + &" ".repeat(cwidth as usize - l);
                }
                scr.write_string(&title, colpos, rowpos);
                colpos += cwidth;
                if colpos >= self.w {
                    break;
                }
            }
            rowpos += 1;
            if rowpos >= self.h {
                break;
            }
        }
        Ok(())
    }
    fn process_key(&mut self, c: char) ->  Transition  {
        Transition::EventPass
    }
    fn enable_command_mode(&mut self, scr: &mut Screen) -> Transition {
        let mode = self.sheets[self.sheet].mode;
        match mode {
            CalcMode::Move => {
                let sheet = &mut self.sheets[self.sheet];
                sheet.mode = CalcMode::Command;
                self.ed_bottom.set_text("");
                self.ed_bottom.on_activate(scr);
                Transition::None
            },
            _ => Transition::None,
        }
    }
    fn enable_temp_range_mode(&mut self, scr: &mut Screen) -> Transition {
        let mode = self.sheets[self.sheet].mode;
        match mode {
            CalcMode::Edit => {
                let sheet = &mut self.sheets[self.sheet];
                sheet.mode = CalcMode::TempSelect;
                self.on_activate(scr);
                Transition::None
            },
            _ => Transition::None,
        }
    }
    fn process_enter(&mut self, scr: &mut Screen, modifiers: KeyModifiers) -> Transition {
        // TODO: return if selected more than 1 cell
        let mode = self.sheets[self.sheet].mode;
        match mode {
            CalcMode::Move => {
                // self.mark_current_cell();
                let sheet = &mut self.sheets[self.sheet];
                let (col, row) = (sheet.cursor.col, sheet.cursor.row);
                info!("--> edit marking {}x{}", col, row);
                sheet.mark_cell(col, row);
                let cell = sheet.cell(col, row);
                sheet.mode = CalcMode::Edit;
                self.ed_top.set_text(&cell.val);
                self.ed_top.on_activate(scr);
                Transition::None
            },
            CalcMode::Edit => {
                let sheet = &mut self.sheets[self.sheet];
                sheet.mode = CalcMode::Move;
                self.ed_top.on_deactivate();
                let (col, row) = (sheet.cursor.col, sheet.cursor.row);
                info!("--> save text {} to {}x{}", self.ed_top.text(), col, row);
                sheet.set_cell_text(col, row, &self.ed_top.text());
                Transition::None
            },
            CalcMode::Select => {
                let sheet = &mut self.sheets[self.sheet];
                sheet.mode = CalcMode::Move;
                sheet.finish_select();
                Transition::None
            },
            CalcMode::Command => {
                if self.run_command(&self.ed_bottom.text()) {
                    return Transition::Exit;
                }
                self.ed_bottom.on_deactivate();
                let sheet = &mut self.sheets[self.sheet];
                sheet.mode = CalcMode::Move;
                Transition::None
            },
            CalcMode::TempSelect => {
                let sheet = &mut self.sheets[self.sheet];
                sheet.mode = CalcMode::Edit;
                let rng = format!("{}", sheet.selected_range());
                self.ed_top.insert(&rng);
                Transition::None
            },
            CalcMode::TempSelectStart => {
                let sheet = &mut self.sheets[self.sheet];
                sheet.mode = CalcMode::Edit;
                sheet.finish_select();
                let rng = format!("{}", sheet.selected_range());
                self.ed_top.insert(&rng);
                Transition::None
            },
        }
    }
    fn process_event_inner(&mut self, ctx: &Context, scr: &mut Screen, event: Event) -> Result<Transition> {
        let sheet = &mut self.sheets[self.sheet];
        let ev = match event {
            Event::Key(ev) => {
                info!("Pressed [{:?}]", ev);
                if self.err.is_some() {
                    self.err = None;
                }
                match ev.code {
                    KeyCode::Esc => match sheet.mode {
                        CalcMode::Move => {
                            Transition::EventPass
                        },
                        CalcMode::TempSelect => {
                            self.ed_top.on_activate(scr);
                            Transition::None
                        },
                        CalcMode::TempSelectStart => {
                            sheet.cancel_select();
                            self.ed_top.on_activate(scr);
                            Transition::None
                        },
                        _ => {
                            sheet.cancel_select();
                            Transition::None
                        },
                    },
                    KeyCode::Left => sheet.arrow_left(ev.modifiers),
                    KeyCode::Right => sheet.arrow_right(ev.modifiers),
                    KeyCode::Down => sheet.arrow_down(ev.modifiers),
                    KeyCode::Up => sheet.arrow_up(ev.modifiers),
                    KeyCode::Home => sheet.go_home(ev.modifiers),
                    KeyCode::End => sheet.go_end(ev.modifiers),
                    KeyCode::PageDown => sheet.page_down(ev.modifiers),
                    KeyCode::PageUp => sheet.page_up(ev.modifiers),
                    KeyCode::Enter => self.process_enter(scr, ev.modifiers),
                    // Delete // TODO: clean a cell or all selected
                    KeyCode::F(2) => if let CalcMode::Move = sheet.mode {
                        if ev.modifiers == KeyModifiers::NONE {
                            self.process_enter(scr, ev.modifiers)
                        } else {
                            Transition::EventPass
                        }
                    } else {
                        Transition::EventPass
                    },
                    KeyCode::Delete => if ev.modifiers == KeyModifiers::NONE {
                        sheet.clear_range();
                        Transition::None
                    } else {
                        Transition::EventPass
                    },
                    KeyCode::Char(c) => match c {
                        'h' | 'H' => sheet.arrow_left(ev.modifiers),
                        'l' | 'L' => sheet.arrow_right(ev.modifiers),
                        'k' | 'K' => sheet.arrow_up(ev.modifiers),
                        'j' | 'J' => sheet.arrow_down(ev.modifiers),
                        // TODO: automatically set text to '=' if empty
                        '=' => if ev.modifiers == KeyModifiers::NONE {
                            if let CalcMode::Move = sheet.mode {
                                self.process_enter(scr, ev.modifiers)
                            } else {
                                Transition::EventPass
                            }
                        } else if ev.modifiers == KeyModifiers::ALT {
                            sheet.resize_col(sheet.cursor.col, 1);
                            Transition::None
                        } else {
                            Transition::EventPass
                        },
                        ':' => if ev.modifiers == KeyModifiers::SHIFT {
                            self.enable_command_mode(scr)
                        } else {
                            Transition::EventPass
                        },
                        '-' => if ev.modifiers == KeyModifiers::ALT {
                            sheet.resize_col(sheet.cursor.col, -1);
                            Transition::None
                        } else {
                            Transition::EventPass
                        },
                        '*' => if ev.modifiers == KeyModifiers::ALT|KeyModifiers::SHIFT {
                            sheet.autosize_col(sheet.cursor.col);
                            Transition::None
                        } else {
                            Transition::EventPass
                        },
                        _ => {
                            Transition::EventPass
                        },
                    },
                    _ => Transition::EventPass,
                }
            },
            _ => { // TODO
                Transition::EventPass
            },
        };
        Ok(ev)
    }
    fn reset(&mut self) {
        self.name = MAIN_WIDGET.to_string();
        self.col = 0;
        self.row = 0;
        self.sheets = vec![Sheet::new(0, self.w, self.h)];
        self.sheet = 0;
        self.gen = 0;
    }
    // Returns true if the application must be closes
    fn run_command(&mut self, cmd: &str) -> bool { // true if app must close // TODO: enum?
        let cmd = cmd.trim();
        let (command, args) = match cmd.find(' ') {
            None => (&cmd[..], ""),
            Some(idx) => (&cmd[..idx], &cmd[idx+1..]),
        };
        match command.to_lowercase().as_str() {
            "reset" => self.reset(),
            // "clear" // TODO: reset only the current page
            "save" | "s" => { // TODO: only "save"?
                let path = args.trim();
                if path.is_empty() { // TODO: allow empty if the file was already saved or loaded
                    self.err = Some("empty file path".to_string());
                    return false;
                }
                if let Err(e) = self.save(&PathBuf::from(path)) {
                    self.err = Some(format!("failed to save to '{}': {:?}", path, e));
                } else {
                    self.err = Some(format!("saved to '{}'", path));
                }
            },
            "load" | "l" => { // TODO: only "load"?
                let path = args.trim();
                if path.is_empty() { // TODO: allow empty to reload
                    self.err = Some("empty file path".to_string());
                    return false;
                }
                if let Err(e) = self.load(&PathBuf::from(path)) {
                    self.err = Some(format!("failed to load from '{}': {:?}", path, e));
                } else {
                    self.err = Some(format!("loaded '{}'", path));
                }
            },
            "q" | "quit" => {
                if self.is_dirty() {
                    self.err = Some("There are unsaved changes. Use 'q!' to quit without saving".to_string());
                    return false;
                }
                return true;
            },
            "q!" | "quit!" => {
                return true;
            },
            "fixrow" => {
                let mut sheet = &mut self.sheets[self.sheet];
                let row = sheet.cursor.row;
                if let Err(e) = sheet.fix_row(row + 1) {
                    self.err = Some(e.to_string());
                }
            },
            "fixcol" => {
                let mut sheet = &mut self.sheets[self.sheet];
                let col = sheet.cursor.col;
                if let Err(e) = sheet.fix_col(col + 1) {
                    self.err = Some(e.to_string());
                }
            },
            "nofixrow" => {
                let mut sheet = &mut self.sheets[self.sheet];
                sheet.unfix_row();
            },
            "nofixcol" => {
                let mut sheet = &mut self.sheets[self.sheet];
                sheet.unfix_col();
            },
            _ => {
                self.err = Some(format!("invalid command '{}'", command));
                info!("Invalid command: {}", command);
            },
        }
        false
    }
    fn save(&mut self, path: &Path) -> Result<()> {
        let f = File::create(path)?;
        serialize_into(&f, &VERSION)?;
        let cnt = self.sheets.len();
        serialize_into(&f, &cnt)?;
        serialize_into(&f, &self.sheet)?;
        let reserv = 0usize;
        serialize_into(&f, &reserv)?;
        for sheet in &self.sheets {
            sheet.save(&f)?;
        }
        for sheet in self.sheets.iter_mut() {
            sheet.dirty = false;
        }
        Ok(())
    }
    fn load(&mut self, path: &Path) -> Result<()> {
        let f = File::open(path)?;
        let mut calc = Calc::default();
        let v: u16 = deserialize_from(&f)?;
        if v != VERSION {
            return Err(anyhow!("unsupported version {}. Expected {}", v, VERSION)); // TODO:
        }
        let sheets: usize = deserialize_from(&f)?;
        if sheets == 0 || sheets > 100 {
            return Err(anyhow!("invalid number of sheets: {}", sheets)); // TODO: 100?
        }
        let curr_sheet: usize = deserialize_from(&f)?;
        if curr_sheet >= sheets {
            return Err(anyhow!("invalid sheet index: {}. Must be within 0:{}", curr_sheet, sheets-1));
        }
        let reserv: usize = deserialize_from(&f)?;
        if reserv != 0usize { // TODO:
            return Err(anyhow!("reserved field must be 0"));
        }
        for _i in 0..sheets {
            let mut sheet = Sheet::load(&f, self.w, self.h, v)?;
            sheet.ensure_visible_col();
            sheet.ensure_visible_row();
            calc.sheets.push(sheet);
        }
        self.sheet = calc.sheet;
        self.sheets = calc.sheets;
        Ok(())
    }
    fn is_dirty(&self) -> bool {
        for sheet in &self.sheets {
            if sheet.dirty {
                return true;
            }
        }
        false
    }
}

impl Widget for Calc {
    fn draw(&self, ctx: &Context, scr: &mut Screen/* , theme: &dyn Theme */) -> Result<()> {
        self.draw_header(ctx, scr)?;
        self.draw_cells(ctx, scr)?;
        self.draw_mode(ctx, scr)?;
        match  self.sheets[self.sheet].mode {
            CalcMode::Edit | CalcMode::TempSelect | CalcMode::TempSelectStart => self.ed_top.draw(ctx, scr),
            CalcMode::Command => self.ed_bottom.draw(ctx, scr),
            _ => self.show_info(ctx, scr),
        }
    }
    fn process_event(&mut self, ctx: &Context, scr: &mut Screen, event: Event) -> Result<Transition> {
        let mode = self.sheets[self.sheet].mode;
        let ev = if let CalcMode::Edit = mode {
            self.ed_top.process_event(ctx, scr, event)?
        } else if let CalcMode::Command = mode {
            self.ed_bottom.process_event(ctx, scr, event)?
        } else {
            Transition::EventPass
        };
        if let Transition::TempSelect = ev {
            Ok(self.enable_temp_range_mode(scr))
        } else if let Transition::EventPass = ev {
            self.process_event_inner(ctx, scr, event)
        } else {
            Ok(ev)
        }
    }
    fn want_tab(&self) -> bool {
        false
    }
    fn on_activate(&mut self, scr: &mut Screen) {
    }
    fn on_deactivate(&mut self) {
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn text(&self) -> String {
        self.name.clone()
    }
    fn set_text(&mut self, _t: &str) {}
    fn gen(&self) -> usize { self.gen }
    fn set_gen(&mut self, gen: usize) { self.gen = gen; }
    fn show(&mut self) {}
    fn hide(&mut self) {}
}
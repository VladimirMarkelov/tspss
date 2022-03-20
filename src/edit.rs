use anyhow::{/* anyhow,  */Result};
use crossterm::{ style::{ Color} };
use crossterm::event::{KeyCode, Event, KeyModifiers};
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::primitive::Screen;
use crate::ui::{Widget,Context,Transition,Dialog,Msg};
use crate::strs;

const MAX_CELL_LEN: usize = 256;

pub struct Edit {
    name: String,
    col: u16,
    row: u16,
    w: u16,
    fg: Color,
    bg: Color,
    text: String,
    active: bool,
    gen: usize,
    command: Dialog,
    visible: bool,
    cursor_pos: u16,
    first_char: u16,
}

impl Edit {
    pub fn new(ctx: &Context, name: &str, col: u16, row: u16, w: u16, fg: Color, bg: Color, text: &str) -> Edit {
        Edit {name: name.to_string(), col, row, w, fg, bg, text: text.to_string(),
            active:false,gen: 0, command: Dialog::None, visible: true, cursor_pos: 0, first_char: 0}
    }
    pub fn with_command(self, command: Dialog) -> Edit {
        Edit {
            command, ..self
        }
    }
    fn visible_text(&self) -> String {
        if self.text.width() > self.w.into() {
            strs::cut(&self.text, self.first_char.into(), self.w.into())
        } else {
            self.text.clone()
        }
    }
    pub fn insert(&mut self, txt: &str) {
        // TODO: too many u16/usize conversions
        let mx = self.text.width() as u16;
        let w = txt.width();
        if w + mx as usize > MAX_CELL_LEN { // TODO: error?
            return;
        }
        if self.cursor_pos + self.first_char >= mx {
            // Adding to the end
            self.text += txt;
        } else {
            let ch = self.text.chars();
            let cnt: usize = (self.cursor_pos+self.first_char).into();
            let mut s: String = ch.take(cnt).collect();
            s += txt;
            let rest:String =  self.text.chars().skip(cnt).collect();
            self.text = s + &rest;
        }
        if w + self.cursor_pos as usize <= self.w as usize {
            self.cursor_pos += w as u16;
            return;
        }
        let diff = w + self.cursor_pos as usize - self.w as usize;
        self.first_char += diff as u16;
        self.cursor_pos = self.w;
    }
}

impl Widget for Edit {
    fn draw(&self, ctx: &Context, scr: &mut Screen/* , theme: &dyn Theme */) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        scr.colors(self.fg, self.bg);
        scr.fill_rect(self.col, self.row, self.w, 1, ' ');

        let txt = self.visible_text();
        scr.write_string(&txt, self.col, self.row);
        if self.active {
            // scr.move_to(self.col+txt.width() as u16, self.row);
            scr.move_to(self.col+self.cursor_pos as u16, self.row);
        }
        Ok(())
    }
    // TODO: check modifiers
    // TODO: clipboard
    fn process_event(&mut self, ctx: &Context, scr: &mut Screen, event: Event) -> Result<Transition> {
        match event {
            Event::Key(ev) => match ev.code {
                KeyCode::Esc => return Ok(Transition::EventPass),
                KeyCode::Left => if self.cursor_pos != 0 {
                    self.cursor_pos-=1;
                } else if self.first_char != 0 {
                    self.first_char -= 1;
                },
                KeyCode::Right => {
                    let mx = self.text.width() as u16;
                    if self.cursor_pos < self.w && self.cursor_pos+self.first_char < mx {
                        self.cursor_pos+=1;
                    } else if self.cursor_pos == self.w && self.cursor_pos+self.first_char< mx {
                        self.first_char += 1;
                    }
                },
                KeyCode::Home => {
                    self.cursor_pos = 0;
                    self.first_char = 0;
                },
                KeyCode::End => {
                    let w = self.text.width() as u16;
                    if w < self.w {
                        self.first_char = 0;
                        self.cursor_pos = w;
                    } else {
                        self.first_char = w - self.w;
                        self.cursor_pos = self.w;
                    }
                },
                KeyCode::Delete => {
                    let w = self.text.width();
                    if usize::from(self.cursor_pos+self.first_char) >= w {
                        return Ok(Transition::None);
                    }
                    let s: Vec<char> = self.text.chars().collect();
                    self.text = s.into_iter().enumerate().filter_map(|(i, e)| if i as u16 != self.first_char+self.cursor_pos { Some(e) } else {None}).collect();
                    let w = self.text.width();
                    if w > self.w.into() && self.first_char != 0 {
                        self.first_char -= 1;
                        if self.first_char+self.cursor_pos-1 < self.text.width() as u16 {
                            self.cursor_pos+=1;
                        }
                    }
                    return Ok(Transition::None);
                },
                KeyCode::Backspace => {
                    if self.first_char+self.cursor_pos != 0 {
                        let s: Vec<char> = self.text.chars().collect();
                        self.text = s.into_iter().enumerate().filter_map(|(i, e)| if i as u16 != self.first_char+self.cursor_pos-1 { Some(e) } else {None}).collect();
                        if self.cursor_pos != 0 {
                            self.cursor_pos-=1;
                        } else if self.first_char != 0 {
                            self.first_char-=1;
                        }
                        if self.first_char != 0 && self.first_char+self.w > self.text.width() as u16 {
                            self.first_char-=1;
                            self.cursor_pos+=1;
                        }
                    }
                    return Ok(Transition::None);
                },
                KeyCode::Enter => if let Dialog::None = self.command {
                    return Ok(Transition::EventPass);
                } else {
                    return Ok(Transition::Pop(Msg::Ok(self.command.clone())));
                },
                KeyCode::Char(c) => if ev.modifiers == KeyModifiers::NONE || ev.modifiers == KeyModifiers::SHIFT {
                    if self.text.width() >= MAX_CELL_LEN {
                        return Ok(Transition::None);
                    }
                    let mut s: Vec<char> = self.text.chars().collect();
                    s.insert((self.first_char+self.cursor_pos) as usize, c);
                    self.text = s.into_iter().collect();
                    if self.w == self.cursor_pos {
                        self.first_char+=1;
                    } else {
                        self.cursor_pos+=1;
                    }
                    return Ok(Transition::None);
                } else if ev.modifiers == KeyModifiers::CONTROL {
                    match c {
                        'r' | 'R' => { // TODO: remove this feature or use different hotkey?
                            self.text = String::new();
                            self.cursor_pos = 0;
                            return Ok(Transition::None);
                        },
                        'p' => {/* TODO: insert from clipboard? or marked range? */},
                        _=> {},
                    }
                } else if ev.modifiers == KeyModifiers::ALT {
                    match c {
                        's' => {
                            return Ok(Transition::TempSelect); // TODO: do it only for edit, not for command
                        },
                        _ => {},
                    }
                },
                _ => {},
            },
            _ => {},
        }
        Ok(Transition::None)
    }
    fn want_tab(&self) -> bool {
        false
    }
    fn on_activate(&mut self, scr: &mut Screen) {
        // TODO:
        self.bg = Color::Grey;
        self.active = true;
    }
    fn on_deactivate(&mut self) {
        // TODO:
        self.bg = Color::DarkGrey;
        self.active = true;
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn text(&self) -> String { self.text.clone() }
    fn set_text(&mut self, t: &str) {
        self.text = t.to_string();
        let w = t.width() as u16;
        if w < self.w {
            self.first_char = 0;
            self.cursor_pos = w;
        } else {
            self.first_char = w - self.w;
            self.cursor_pos = self.w;
        }
    }
    fn gen(&self) -> usize { self.gen }
    fn set_gen(&mut self, gen: usize) { self.gen = gen; }
    fn show(&mut self) { self.visible = true; }
    fn hide(&mut self) { self.visible = false; }
}

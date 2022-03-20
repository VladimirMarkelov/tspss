use anyhow::{/* anyhow,  */Result};
use crossterm::{ style::{ Color} };
use crossterm::event::{KeyCode, Event};

use crate::primitive::Screen;
use crate::ui::{Widget,Context,Transition,Dialog,Msg,NOTHING,Command};
use crate::strs;

pub struct ListItem {
    text: String,
    submenu: Option<Dialog>,
    command: Command,
}

impl ListItem {
    pub fn new(s: &str, cmd: Command) -> ListItem {
        ListItem{text: s.to_string(), submenu: None, command: cmd}
    }
    pub fn new_submenu(s: &str, dlg: Dialog) -> ListItem {
        ListItem{text: s.to_string(), submenu: Some(dlg), command: Command::None}
    }
}

pub struct ListBox {
    name: String,
    col: u16,
    row: u16,
    w: u16,
    h: u16,
    fg: Color,
    bg: Color,
    selected: usize,
    top: usize,
    items: Vec<ListItem>,
    visible: bool,
    gen: usize,
}

impl ListBox {
    pub fn new(ctx: &Context, name: &str, col: u16, row: u16, w: u16, h: u16, fg: Color, bg: Color) -> ListBox {
        ListBox {
            name: name.to_string(), col, row, w, h, fg, bg,
            selected: 0, top: 0, items: Vec::new(), gen: 0, visible: true,
        }
    }
    pub fn push_item(&mut self, item: ListItem) {
        self.items.push(item);
    }
    pub fn set_selected(&mut self, idx: usize) {
        if idx >= self.items.len() {
            return;
        }
        self.selected = idx;
    }
}

impl Widget for ListBox {
    fn draw(&self, ctx: &Context, scr: &mut Screen/* , theme: &dyn Theme */) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        let has_submenu = self.items.iter().any(|ref x| x.submenu.is_some());
        let long = self.items.len() > self.h as usize;
        let mut width = self.w;
        if has_submenu { width -= 1 }
        if long { width -= 1 }
        scr.colors(self.fg, self.bg);
        scr.fill_rect(self.col, self.row, self.w, self.h, ' ');
        for row in self.row..self.row+std::cmp::min(self.h, self.items.len() as u16) {
            let item = &self.items[(row-self.row) as usize +self.top];
            let mut txt = strs::pad(&strs::cut(&(*item).text, 0, width as usize), self.w.into());
            let rpos = if long {
                txt.insert(0, ' ');
                self.col + 1
            } else {
                self.col
            };
            if (row -self.row+self.top as u16) as usize == self.selected {
                scr.colors(self.bg, self.fg);
            } else {
                scr.colors(self.fg, self.bg);
            }
            scr.write_string(&txt, rpos, row);
            if item.submenu.is_some() {
                scr.write_string(">", self.col+self.w-1, row);
            }
        }
        if long {
            if self.top != 0 {
                if self.top == self.selected {
                    scr.colors(self.bg, self.fg);
                } else {
                    scr.colors(self.fg, self.bg);
                }
                scr.write_string("^", self.col, self.row);
            }
            if (self.top + self.h as usize) < self.items.len() {
                if self.selected == self.top + self.h as usize {
                    scr.colors(self.bg, self.fg);
                } else {
                    scr.colors(self.fg, self.bg);
                }
                scr.write_string("v", self.col, self.row+self.h-1);
            }
        }
        Ok(())
    }
    fn process_event(&mut self, ctx: &Context, scr: &mut Screen, event: Event) -> Result<Transition> {
        match event {
            Event::Key(ev) => match ev.code {
                KeyCode::Up => {
                    if self.selected != 0 {
                        if self.top == self.selected {
                            self.top -= 1;
                        }
                        self.selected -= 1;
                    }
                    Ok(Transition::None)
                },
                KeyCode::Down => {
                    if self.selected != self.items.len() - 1 {
                        if self.selected - self.top == (self.h-1).into() {
                            self.top += 1;
                        }
                        self.selected += 1;
                    }
                    Ok(Transition::None)
                },
                KeyCode::Enter => {
                    if self.selected == NOTHING {
                        Ok(Transition::None)
                    } else {
                        let item = &self.items[self.selected];
                        if let Some(menu) = &item.submenu {
                            Ok(Transition::Push(menu.clone()))
                        } else {
                            Ok(Transition::Pop(Msg::Cmd(item.command)))
                        }
                    }
                },
                /*
                KeyCode::PageUp => {
                    Ok(Transition::None)
                },
                KeyCode::PageDown => {
                    Ok(Transition::None)
                },
                */
                _ => Ok(Transition::EventPass),
            },
            _ => Ok(Transition::EventPass),
        }
    }
    fn want_tab(&self) -> bool {
        true
    }
    fn on_activate(&mut self, scr: &mut Screen) {
        // TODO:
        self.fg = Color::Green;
    }
    fn on_deactivate(&mut self) {
        // TODO:
        self.fg = Color::White;
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
    fn show(&mut self) { self.visible = true; }
    fn hide(&mut self) { self.visible = false; }
    fn on_command(&mut self, cmd: Msg) -> Result<Transition> { Ok(Transition::EventPass) }
}

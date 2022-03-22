use anyhow::{/* anyhow,  */Result};
use crossterm::{ style::{ Color} };
use crossterm::event::{KeyCode, Event};

use crate::primitive::{Screen, Border};
use crate::ui::{Widget,Context,Transition, Msg};

pub struct Panel {
    name: String,
    col: u16,
    row: u16,
    w: u16,
    h: u16,
    fg: Color,
    bg: Color,
    border: Border,
    gen: usize,
    visible: bool,
}

impl Panel {
    // TODO: too many args
    pub fn new(ctx: &Context, name: &str, col: u16, row: u16, w: u16, h: u16, fg: Color, bg: Color, border: Border) -> Panel {
        Panel {name: name.to_string(), col, row, w, h, fg, bg, border, gen: 0, visible: true,}
    }
}

impl Widget for Panel {
    fn draw(&self, ctx: &Context, scr: &mut Screen/* , theme: &dyn Theme */) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        scr.colors(self.fg, self.bg);
        scr.draw_frame(self.col, self.row, self.w, self.h, self.border);
        scr.fill_rect(self.col+1, self.row+1, self.w-2, self.h-2, ' ');
        Ok(())
    }
    fn process_event(&mut self, ctx: &Context, scr: &mut Screen, event: Event) -> Result<Transition> {
        if let Event::Key(ek) = event {
            match ek.code {
                KeyCode::Esc => return Ok(Transition::Pop(Msg::None)),
                KeyCode::Tab => return Ok(Transition::EventPass),
                _ => return Ok(Transition::None),
            }
        }
        Ok(Transition::None)
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
    fn show(&mut self) { self.visible = true; }
    fn hide(&mut self) { self.visible = false; }
    fn on_command(&mut self, _cmd: Msg) -> Result<Transition> { Ok(Transition::EventPass) }
}

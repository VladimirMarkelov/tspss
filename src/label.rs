use anyhow::{/* anyhow,  */Result};
use crossterm::{ style::{ Color} };
use crossterm::event::{KeyCode, Event};

use crate::primitive::Screen;
use crate::ui::{Widget,Context,Transition};

pub struct Label {
    name: String,
    col: u16,
    row: u16,
    fg: Color,
    bg: Color,
    text: String,
    gen: usize,
    visible: bool,
}

impl Label {
    pub fn new(ctx: &Context, name: &str, col: u16, row: u16, fg: Color, bg: Color, text: &str) -> Label {
        Label {name: name.to_string(), col, row, fg, bg, text: text.to_string(), gen: 0, visible: true}
    }
}

impl Widget for Label {
    fn draw(&self, ctx: &Context, scr: &mut Screen/* , theme: &dyn Theme */) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        scr.colors(self.fg, self.bg);
        scr.write_string(&self.text, self.col, self.row);
        Ok(())
    }
    fn process_event(&mut self, ctx: &Context, scr: &mut Screen, event: Event) -> Result<Transition> {
        Ok(Transition::EventPass)
    }
    fn want_tab(&self) -> bool {
        true // TODO: fix after debug
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
}

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use crossterm::event::{Event, KeyCode};
use unicode_width::UnicodeWidthStr;
use crossterm::style::{Color};

use crate::primitive::{Screen,Border};
use crate::panel::Panel;
use crate::listbox::{ListBox, ListItem};
use crate::label::Label;

pub const MAIN_WIDGET: &str = "calc";
pub const NOTHING: usize = -1i64 as usize;

#[derive(Clone,Debug)]
pub struct PageListArgs {
    pub items: Vec<String>,
    pub default: String,
    pub title: String,
}

#[derive(Debug,Clone)]
pub enum Dialog {
    None,
    PageList(PageListArgs),
}

#[derive(Debug,Copy,Clone)]
pub enum Command {
    None,
    Page_ID(usize),
}

#[derive(Debug,Clone)]
pub enum Msg {
    // Str(String),
    Cmd(Command),
    // Int(i64),
    // Chr(char),
    // Cursor(u16,u16),
    Ok(Dialog),
    None,
}

#[derive(Debug)]
pub enum Transition {
    None,  // Do nothing
    EventPass, // Event processed stop passing to the parent
    Pop(Msg), // Close the last dialog, Msg contains the DialogResult
    Exit, // Exit app
    Push(Dialog), // Create a new dialog with type Dialog
    TempSelect, // Start selection - limited mode which allows a user only selecting and escaping a range
}

pub struct Context {
    pub w: u16,
    pub h: u16,
}

impl Context {
    pub fn new(w: u16, h: u16) -> Context {
        Context{w, h}
    }
}

pub trait Widget {
    fn draw(&self, ctx: &Context, scr: &mut Screen/* , theme: &dyn Theme */) -> Result<()> ;
    fn process_event(&mut self, ctx: &Context, scr: &mut Screen, event: Event) -> Result<Transition>;
    fn want_tab(&self) -> bool;
    fn on_activate(&mut self, scr: &mut Screen);
    fn on_deactivate(&mut self);
    fn name(&self) -> &str;
    fn text(&self) -> String;
    fn set_text(&mut self, t: &str);
    fn gen(&self) -> usize;
    fn set_gen(&mut self, gen: usize);
    fn hide(&mut self);
    fn show(&mut self);
    fn on_command(&mut self, cmd: Msg) -> Result<Transition>;
}

pub struct WidgetStack {
    gen: usize,
    focused: usize,
    widgets: Vec<Box<dyn Widget>>,
    last_result: HashMap<String, String>,
}

impl Default for WidgetStack {
    fn default() -> WidgetStack {
        WidgetStack {
            gen: 0,
            focused: NOTHING,
            widgets: Vec::new(),
            last_result: HashMap::new(),
        }
    }
}

impl WidgetStack {
    fn last_gen(&self) -> usize {
        if let Some(item) = self.widgets.last() {
            item.gen()
        } else {
            NOTHING
        }
    }
    pub fn next_gen(&mut self) {
        if let Some(item) = self.widgets.last() {
            self.gen = item.gen() + 1;
        }
    }
    pub fn push(&mut self, mut w: Box<dyn Widget>) {
        if self.focused != NOTHING {
            self.widgets[self.focused].on_deactivate();
        }
        self.focused = NOTHING;
        w.set_gen(self.gen);
        self.widgets.push(w);
    }
    fn save_result(&mut self) {
        self.last_result.clear();
        let gen = self.last_gen();
        if gen == NOTHING {
            return;
        }
        for w in self.widgets.iter().rev() {
            // TODO: skip static items
            if w.gen() < gen {
                break;
            }
            assert_eq!(w.gen(), gen);
            self.last_result.insert(w.name().to_string(), w.text());
        }
    }
    pub fn pop(&mut self) {
        let gen = self.last_gen();
        if gen == NOTHING {
            return;
        }
        self.focused = NOTHING; // TODO: remember previously focused
        self.widgets.retain(|w| w.gen() < gen);
        self.gen -= 1;
    }
    // Move focus to the next widget in the current dialog. Return true if the focus changed.
    // TODO: optimize?
    pub fn focus_next(&mut self, scr: &mut Screen) -> bool {
        let gen = self.last_gen();
        if gen == NOTHING {
            return false;
        };
        let mut first = NOTHING;
        let curr = self.focused;
        let mut selected = NOTHING;
        for (idx, w) in self.widgets.iter_mut().enumerate() {
            if !w.want_tab() || w.gen() != gen {
                continue;
            }
            if first == NOTHING {
                first = idx;
            }
            if curr == NOTHING {
                selected = idx;
                break;
            }
            if curr < idx {
                selected = idx;
                break;
            }
        }
        if selected == NOTHING {
            selected = first;
        }
        if selected == NOTHING || selected == curr {
            return false;
        }
        if curr != NOTHING {
            self.widgets[curr].on_deactivate();
        }
        info!("focus next to {}", self.widgets[selected].name());
        self.widgets[selected].on_activate(scr);
        self.focused = selected;
        true
    }
    pub fn draw(&self, ctx: &Context, scr: &mut Screen) -> Result<()> {
        let gen = self.last_gen();
        if gen == NOTHING {
            return Ok(());
        }
        for w in self.widgets.iter() {
            if w.gen() != gen {
                continue;
            }
            w.draw(ctx, scr)?;
        }
        Ok(())
    }
    pub fn process_event(&mut self, ctx: &Context, scr: &mut Screen, event: Event) -> Result<Transition> {
        if self.focused == NOTHING {
            return Ok(Transition::EventPass);
        }
        let r = self.widgets[self.focused].process_event(ctx, scr, event)?;
        match r {
            Transition::EventPass => {
                if let Event::Key(ek) = event {
                    if ek.code == KeyCode::Tab && self.focus_next(scr) {
                        self.draw(ctx, scr)?;
                    } else if ek.code == KeyCode::Esc {
                        info!("Wstack ESC {:?}", ek);
                        if self.is_main_dlg() {
                            return Ok(Transition::Exit);
                        } else {
                            self.last_result.clear();
                            self.pop();
                            if self.is_main_dlg() {
                                // self.focus_next(scr)?; // TODO: remember previously focused
                                self.set_focus(MAIN_WIDGET, scr)?;
                            }
                            return Ok(Transition::None);
                        }
                    }
                }
                Ok(Transition::None)
            },
            Transition::Pop(msg) => {
                info!("Pop {:?}", msg);
                if self.is_main_dlg() {
                    Ok(Transition::Exit)
                } else {
                    self.save_result();
                    self.pop();
                    if self.is_main_dlg() {
                        // self.focus_next(scr)?; // TODO: remember previously focused
                        self.set_focus(MAIN_WIDGET, scr)?;
                        self.pass_command(MAIN_WIDGET, msg)?; // TODO: use transition or make it Result<()>?
                    }
                    // match msg {
                    //     _ => {}, // TODO: process dialog close events and update other widgets
                    // }
                    Ok(Transition::None)
                }
            },
            Transition::Push(dlg) => {
                info!("New dialog {:?}", dlg);
                match dlg {
                    Dialog::PageList(args) => {
                        if args.items.is_empty() {
                            return Err(anyhow!(format!("list for '{}' is empty", args.title)));
                        }
                        info!("select a page from list");
                        let mut mx = args.title.width();
                        let mut selected: usize = 0;
                        for (idx,item) in args.items.iter().enumerate() {
                            let w = item.width();
                            if w > mx {
                                mx = w;
                            }
                            if item.as_str() == args.default.as_str() {
                                selected = idx;
                            }
                        }
                        mx += 2;
                        if mx > ctx.w as usize -4 {
                            mx = ctx.w as usize - 4;
                        }
                        let h = if args.items.len() > ctx.h as usize - 6 { ctx.h - 6 } else { args.items.len() as u16 + 2};
                        let w = mx as u16 + 2;
                        let posx = ctx.w/2 - w/2;
                        let posy = ctx.h/2 - h/2;
                        let panel = Box::new(Panel::new(ctx, "p", posx, posy, w, h, Color::White, Color::Black, Border::Single));
                        let lbl = Box::new(Label::new(ctx, "lbl", posx+1, posy, Color::White, Color::Black, &args.title)); // TODO: cut title if long
                        let mut lbx = Box::new(ListBox::new(ctx, "lbx", posx+1, posy+1, w-2, h-2, Color::DarkBlue, Color::Blue));
                        for (idx, item) in args.items.iter().enumerate() {
                            lbx.push_item(ListItem::new(item, Command::Page_ID(idx)));
                        }
                        lbx.set_selected(selected);
                        self.next_gen();
                        self.push(panel);
                        self.push(lbl);
                        self.push(lbx);
                        self.set_focus("lbx", scr)?;
                    },
                    _ => info!("unimplemented dialog {:?}", dlg),
                }
                Ok(Transition::None)
            },
            _ => Ok(r),
        }
    }
    pub fn is_main_dlg(&self) -> bool {
        self.last_gen() == 0
    }
    pub fn set_focus(&mut self, name: &str, scr: &mut Screen) -> Result<()> {
        let mut found  = false;
        let gen = self.last_gen();
        if gen == NOTHING {
            return Err(anyhow!("no dialogs"));
        }
        for (idx, w) in self.widgets.iter_mut().enumerate() {
            if w.gen() != gen || w.name() != name {
                continue;
            }
            if idx != self.focused {
                w.on_activate(scr);
            }
            self.focused = idx;
            found = true;
            break;
        }
        if !found {
            return Err(anyhow!("Widget {} not found", name));
        }
        Ok(())
    }
    fn pass_command(&mut self, name: &str, cmd: Msg) -> Result<Transition> {
        let gen = self.last_gen();
        if gen == NOTHING {
            return Err(anyhow!("no dialogs"));
        }
        for (idx, w) in self.widgets.iter_mut().enumerate() {
            if w.gen() != gen || w.name() != name {
                continue;
            }
            return w.on_command(cmd);
        }
        Err(anyhow!("Widget {} not found", name))
    }
}


#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;

mod buffer;
mod primitive;
mod strs;

mod ui;
mod panel;
mod label;
mod listbox;
mod edit;
mod calc;
mod sheet;
mod parse;
mod ops;
mod stack;
mod expr;

use std::fs::File;
use std::io::{stdin, stdout, Write};

use anyhow::{anyhow, Result};
use crossterm::event::{read, /* EnableMouseCapture, */KeyCode,Event};
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode, ClearType};
use crossterm::tty::IsTty;
use crossterm::{
    cursor, execute, queue,
    style::{self, Color},
};
use simplelog::*;

use primitive::Screen;
use ui::{WidgetStack,Context,Widget,Dialog,Transition,Msg,NOTHING,Command};
use panel::Panel;
use label::Label;
use edit::Edit;
use calc::Calc;

// fn scr_reset(scr: &mut Screen) {
//     scr.clear();
//     scr.colors(Color::White, Color::Black);
// }

fn create_main_dlg(ctx: &Context, wstack: &mut WidgetStack) {
    // let mut lb = Box::new(listbox::ListBox::new(ctx, "lb", 20, 5, 10, 5, Color::DarkBlue, Color::Blue));
    // lb.push_item(listbox::ListItem::new("First"));
    // lb.push_item(listbox::ListItem::new("Second"));
    // lb.push_item(listbox::ListItem::new("Third"));
    // lb.push_item(listbox::ListItem::new_submenu("Fourth", Dialog::PopupMenu));
    // lb.push_item(listbox::ListItem::new("Fifth"));
    // lb.push_item(listbox::ListItem::new("Sixth"));
    // lb.push_item(listbox::ListItem::new("Seventh"));
    wstack.push(Box::new(Calc::new(ctx)));
    // wstack.push(Box::new(Panel::new(ctx, "p", 10, 5, 16, 6, Color::Grey)));
    // wstack.push(Box::new(Label::new(ctx, "l1", 11, 8, Color::White, Color::Grey, "Line1")));
    // wstack.push(Box::new(Label::new(ctx, "l2", 11, 10, Color::White, Color::Grey, "Line2")));
    // wstack.push(lb);
    // wstack.push(Box::new(Edit::new(ctx, "ed1", 10, 2, 8, Color::Black, Color::White, "test")));
    // let mut ed_top = Box::new(Edit::new(ctx, "ed-top", 0, 0, ctx.w-1, Color::White, Color::Black, "[TOP]ed-top"));
    // ed_top.hide();
    // wstack.push(ed_top);
    // let mut ed_bottom = Box::new(Edit::new(ctx, "ed-bottom", 0, ctx.h-1, ctx.w-1, Color::White, Color::Black, "[BTM]ed-bottom"));
    // ed_bottom.hide();
    // wstack.push(ed_bottom);
}

fn main_loop(/*cli: &opts::CliOpts*/) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    let /* mut */ ctx = Context::new(cols, rows);
    let mut scr = Screen::new(cols, rows)?;
    let mut stdout = stdout();
    if !stdin().is_tty() {
        return Err(anyhow!("stdin is not TTY"));
    }
    // execute!(stdout, EnableMouseCapture)?;

    /*
    let filename = if cli.filename.is_empty() { None } else { Some(cli.filename.clone()) };
    let rules = rules::load_rules(filename)?;
    info!("Loaded from {:?} - {}", &cli.filename, rules.len());

    let mut user_conf = userconf::UserConf::load();
    if user_conf.last_played.is_empty() {
        let mut sols: Vec<String> = Vec::new();
        for (name, _cfg) in rules.iter() {
            sols.push(name.clone());
        }
        if !sols.is_empty() {
            sols.sort();
            user_conf.last_played = sols[0].clone();
        }
    }
    ctx.name = user_conf.last_played.clone();
    ctx.custom = !cli.filename.is_empty();

    let mut stg: Box<dyn Strategy> = Box::new(ChooseStg::new(&rules, &mut ctx)?);
    let mut stages: Vec<Box<dyn Strategy>> = Vec::new();

    let dark = theme::DarkTheme::new(!cli.four_color);
    let light = theme::LightTheme::new(!cli.four_color);
    let thm: &dyn theme::Theme = if cli.dark { &dark } else { &light };
    let (fg, bg) = thm.base_colors();
    */
    let fg = Color::White;
    let bg = Color::Black;
    scr.colors(fg, bg);
    scr.clear();
    execute!(stdout, style::SetForegroundColor(fg), style::SetBackgroundColor(bg), terminal::Clear(ClearType::All),)?;
    let mut wstack: WidgetStack = Default::default();
    create_main_dlg(&ctx, &mut wstack);
    wstack.set_focus("calc", &mut scr)?;

    // Draw first time out of the loop to simplify even+change+redraw loop
    scr.colors(Color::White, Color::Black);
    wstack.draw(&ctx, &mut scr)?;
    scr.flush(&mut stdout)?;
    stdout.flush()?;

    loop {
        let ev = read()?;
        let mut r = wstack.process_event(&ctx, &mut scr, ev)?;
        if let Transition::EventPass = r {
            info!("Main loop got event {:?}", r);
            match ev {
                Event::Key(ev) => match ev.code {
                    KeyCode::Esc => r = Transition::Pop(Msg::Cmd(Command::None)),
                    _ => {},
                },
                _ => {},
            }
        }

        match r {
            Transition::Exit => return Ok(()),
            _ => {},
        }
        scr.colors(Color::White, Color::Black);
        wstack.draw(&ctx, &mut scr)?;
        scr.flush(&mut stdout)?;
        stdout.flush()?;
        /*
        let trans = stg.process_event(&mut ctx, &mut scr, ev)?;
        match trans {
            Transition::None => {}
            Transition::Pop => match stages.pop() {
                None => return Ok(()),
                Some(s) => {
                    scr_reset(&mut scr);
                    stg = s;
                    stg.on_activate(&mut ctx);
                }
            },
            Transition::Exit => {
                stg.on_deactivate(&mut ctx);
                if ctx.moved {
                    ctx.stats.update_stat(&ctx.name, ctx.won, ctx.elapsed);
                    if !ctx.custom {
                        ctx.stats.save();
                    }
                }
                stages.clear();
                user_conf.last_played = ctx.name.clone();
                if !ctx.custom {
                    user_conf.save();
                }
                return Ok(());
            }
            Transition::Push(st) => {
                stg.on_deactivate(&mut ctx);
                stages.push(stg);
                scr_reset(&mut scr);
                stg = match st {
                    TransitionStage::EndDialog => Box::new(FinalStg::new(&mut ctx)?),
                    TransitionStage::Play => {
                        ctx.state.clear_mark();
                        ctx.state.clear_hints();
                        Box::new(PlayStg::new(&rules, &mut ctx)?)
                    }
                    TransitionStage::Choose => {
                        ctx.state.clear_mark();
                        ctx.state.clear_hints();
                        Box::new(ChooseStg::new(&rules, &mut ctx)?)
                    }
                    TransitionStage::HelpDialog => Box::new(HelpStg::new(&mut ctx)?),
                };
                stg.on_activate(&mut ctx);
            }
            Transition::Replace(st) => {
                stg.on_deactivate(&mut ctx);
                stages.clear();
                if ctx.moved {
                    ctx.stats.update_stat(&ctx.name, ctx.won, ctx.elapsed);
                    if !ctx.custom {
                        ctx.stats.save();
                    }
                }
                ctx.moved = false;
                ctx.won = false;
                scr_reset(&mut scr);
                stg = match st {
                    TransitionStage::EndDialog => Box::new(FinalStg::new(&mut ctx)?),
                    TransitionStage::Play => {
                        ctx.state.clear_mark();
                        ctx.state.clear_hints();
                        Box::new(PlayStg::new(&rules, &mut ctx)?)
                    }
                    TransitionStage::Choose => {
                        ctx.state.clear_mark();
                        ctx.state.clear_hints();
                        Box::new(ChooseStg::new(&rules, &mut ctx)?)
                    }
                    _ => panic!("unimplemented"),
                };
                ctx.reset();
            }
            */
    }
}

fn main() -> Result<()> {
/*
    let cli = opts::parse_args();

    if cli.logging {
*/
        let cb = ConfigBuilder::new().set_time_format("[%Y-%m-%d %H:%M:%S%.3f]".to_string()).build();
        CombinedLogger::init(vec![WriteLogger::new(LevelFilter::Info, cb, File::create("app.log").unwrap())]).unwrap();
/*
    }
  */
    let mut stdout = stdout(); // TODO: maybe let mut stdout = std::io::BufWriter(Stdout::new());
    execute!(stdout, terminal::EnterAlternateScreen)?;
    enable_raw_mode()?;

    // execute!(stdout, cursor::Hide)?; // TODO:
    let err = main_loop(/*&cli*/);

    queue!(stdout, cursor::Show, style::ResetColor, terminal::LeaveAlternateScreen)?;
    stdout.flush()?;

    disable_raw_mode()?;
    // if err.is_err() {
    //     err
    // } else {
    //     Ok(())
    // }
    err
}

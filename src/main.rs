//! bushel — a lazydocker-style TUI for Apple Containers.
//! Elm-style loop: one AppState, one AppEvent channel, single-writer updates;
//! render on event plus a frame ticker armed only while effects are active.

use std::io::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt as _;
use tokio::sync::mpsc;

use bushel::client::Client;
use bushel::config::Config;
use bushel::engine::{Command, Engine};
use bushel::runner::{CONTAINER_BIN, CliRunner};
use bushel::ui::theme::Theme;
use bushel::ui::{Ui, keymap};

#[derive(Parser, Debug)]
#[command(
    name = "bushel",
    version,
    about = "A lazydocker-style TUI for Apple Containers"
)]
struct Args {
    /// Skip the splash screen
    #[arg(long)]
    no_splash: bool,
    /// Disable all animation and effects
    #[arg(long)]
    reduced_motion: bool,
    /// ASCII icons and spinners (no Unicode glyphs)
    #[arg(long)]
    ascii: bool,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let cfg = Config::load();
    let no_splash = args.no_splash || cfg.no_splash;
    let reduced_motion = args.reduced_motion || cfg.reduced_motion;
    let ascii = args.ascii || cfg.ascii;

    let client = Client::new(Arc::new(CliRunner));
    let (tx, mut rx) = mpsc::channel(1024);
    let mut engine = Engine::new(client, tx, no_splash || reduced_motion);
    let mut ui = Ui::new(Theme::detect(ascii), reduced_motion);

    let mut terminal = ratatui::init();
    engine.start();

    let mut keys = EventStream::new();
    let mut poll = tokio::time::interval(Duration::from_secs(1));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_frame = Instant::now();

    let result: std::io::Result<()> = loop {
        // render
        let elapsed = last_frame.elapsed();
        last_frame = Instant::now();
        if let Err(e) = terminal.draw(|f| ui.render(f, &engine.state, elapsed)) {
            break Err(e);
        }

        if engine.state.quit {
            break Ok(());
        }

        // exec: suspend the TUI, hand the terminal to the shell, restore
        if engine.state.exec_request.is_some() {
            let exec_args = engine.prepare_exec();
            ratatui::restore();
            let status = std::process::Command::new(CONTAINER_BIN)
                .args(&exec_args)
                .status();
            let _ = std::io::stdout().flush();
            terminal = ratatui::init();
            let _ = terminal.clear();
            match status {
                Ok(s) if !s.success() => {
                    engine
                        .state
                        .toast(format!("exec exited {}", s.code().unwrap_or(-1)), true);
                }
                Err(e) => engine.state.toast(format!("exec failed: {e}"), true),
                _ => {}
            }
            engine.after_exec();
            ui.after_exec();
            continue;
        }

        // frame ticker armed only while an effect or splash is active
        let frame_timeout = if ui.animating(&engine.state) {
            Duration::from_millis(33)
        } else if ui.ambient_active() {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(3600) // render is event-driven when idle
        };

        tokio::select! {
            _ = tokio::time::sleep(frame_timeout) => {} // repaint for animation
            _ = poll.tick() => engine.on_tick(),
            ev = rx.recv() => {
                match ev {
                    Some(ev) => {
                        engine.apply(ev);
                        // drain whatever else is queued before repainting
                        while let Ok(ev) = rx.try_recv() {
                            engine.apply(ev);
                        }
                    }
                    None => break Ok(()),
                }
            }
            key = keys.next() => {
                match key {
                    Some(Ok(Event::Key(k))) if k.kind == KeyEventKind::Press => {
                        for cmd in keymap::map_key(&engine.state, k, ui.last_info.log_scroll) {
                            engine.dispatch(cmd);
                        }
                    }
                    Some(Ok(_)) => {} // resize etc: just repaint
                    Some(Err(e)) => break Err(e),
                    None => break Ok(()),
                }
            }
        }
    };

    engine.dispatch(Command::Quit);
    engine.shutdown();
    ratatui::restore();
    result
}

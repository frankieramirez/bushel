//! bushel — a lazydocker-style TUI for Apple Containers.
//! Elm-style loop: one AppState, one AppEvent channel, single-writer updates;
//! render on event plus a frame ticker armed only while effects are active.

use std::io::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
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
    #[command(subcommand)]
    command: Option<Cmd>,
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

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Update bushel to the latest release
    Update,
}

/// Homebrew owns binaries under its prefix; self-replacing one desyncs brew's
/// bookkeeping, so hand the upgrade back to brew instead.
fn brew_managed() -> bool {
    std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map(|p| {
            p.components()
                .any(|c| c.as_os_str() == "Cellar" || c.as_os_str() == "homebrew")
        })
        .unwrap_or(false)
}

async fn self_update() -> i32 {
    if brew_managed() {
        println!("bushel was installed via Homebrew; running `brew upgrade bushel`…");
        return match std::process::Command::new("brew")
            .args(["upgrade", "bushel"])
            .status()
        {
            Ok(s) if s.success() => 0,
            Ok(_) => 1,
            Err(e) => {
                eprintln!("failed to run brew: {e}");
                1
            }
        };
    }

    let mut updater = axoupdater::AxoUpdater::new_for("bushel");
    if updater.load_receipt().is_err() {
        eprintln!(
            "no install receipt found — bushel wasn't installed via the shell installer.\n\
             Reinstall with the installer from the latest GitHub release, or use your\n\
             original install method to upgrade."
        );
        return 1;
    }
    match updater.run().await {
        Ok(Some(result)) => {
            println!("updated bushel to {}", result.new_version);
            0
        }
        Ok(None) => {
            println!("bushel {} is already up to date", env!("CARGO_PKG_VERSION"));
            0
        }
        Err(e) => {
            eprintln!("update failed: {e}");
            1
        }
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    if let Some(Cmd::Update) = args.command {
        std::process::exit(self_update().await);
    }
    let cfg = Config::load();
    let no_splash = args.no_splash || cfg.no_splash;
    let reduced_motion = args.reduced_motion || cfg.reduced_motion;
    let ascii = args.ascii || cfg.ascii;

    // very first launch (no marker file yet): the splash gets a dwell
    let first_run = match Config::dir().map(|d| d.join(".launched")) {
        Some(marker) if !marker.exists() => {
            let _ = std::fs::create_dir_all(marker.parent().unwrap())
                .and_then(|()| std::fs::write(&marker, b""));
            true
        }
        Some(_) => false,
        None => false,
    };

    let client = Client::new(Arc::new(CliRunner));
    let (tx, mut rx) = mpsc::channel(1024);
    let mut engine = Engine::new(client, tx, no_splash || reduced_motion);
    engine.state.first_run = first_run && !(no_splash || reduced_motion);
    let mut ui = Ui::new(Theme::detect(ascii), reduced_motion);

    let mut terminal = ratatui::init();
    engine.start();

    let mut keys = EventStream::new();
    let mut poll = tokio::time::interval(Duration::from_secs(1));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_frame = Instant::now();

    let result: std::io::Result<()> = loop {
        // first-run dwell can end between events; the frame ticker keeps us spinning
        engine.maybe_dissolve_splash();
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

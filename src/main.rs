use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser as _;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt as _;
use tokio::sync::mpsc;

use bushel::cli::{Args, Cmd};
use bushel::client::Client;
use bushel::completions;
use bushel::config::Config;
use bushel::engine::{Command, Engine};
use bushel::runner::{CONTAINER_BIN, CliRunner};
use bushel::ui::theme::Theme;
use bushel::ui::{Ui, keymap};

#[derive(Debug, PartialEq, Eq)]
enum InstallMethod {
    Homebrew,
    Nix,
    Cargo,
    Receipt,
}

fn classify(exe: &Path, cargo_bins: &[PathBuf]) -> InstallMethod {
    if exe
        .components()
        .any(|c| c.as_os_str() == "Cellar" || c.as_os_str() == "homebrew")
    {
        return InstallMethod::Homebrew;
    }
    if exe.starts_with("/nix/store") {
        return InstallMethod::Nix;
    }
    if exe
        .parent()
        .is_some_and(|dir| cargo_bins.iter().any(|bin| bin == dir))
    {
        return InstallMethod::Cargo;
    }
    InstallMethod::Receipt
}

fn cargo_bin_candidates(
    install_root: Option<PathBuf>,
    cargo_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Vec<PathBuf> {
    let home_cargo = home.map(|h| h.join(".cargo"));
    [install_root, cargo_home.or(home_cargo)]
        .into_iter()
        .flatten()
        .map(|root| root.join("bin"))
        .collect()
}

fn cargo_bin_dirs() -> Vec<PathBuf> {
    cargo_bin_candidates(
        std::env::var_os("CARGO_INSTALL_ROOT").map(PathBuf::from),
        std::env::var_os("CARGO_HOME").map(PathBuf::from),
        dirs::home_dir(),
    )
    .into_iter()
    .filter_map(|dir| std::fs::canonicalize(dir).ok())
    .collect()
}

fn install_method() -> InstallMethod {
    std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map(|exe| classify(&exe, &cargo_bin_dirs()))
        .unwrap_or(InstallMethod::Receipt)
}

fn brew_upgrade_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("brew");
    cmd.args(["upgrade", "bushel"])
        .env("HOMEBREW_AUTO_UPDATE_SECS", "0")
        .env_remove("HOMEBREW_NO_AUTO_UPDATE");
    cmd
}

async fn self_update() -> i32 {
    let method = install_method();
    match method {
        InstallMethod::Homebrew => {
            println!(
                "bushel was installed via Homebrew; refreshing the tap, then running \
                 `brew upgrade bushel`…"
            );
            return match brew_upgrade_command().status() {
                Ok(s) if s.success() => 0,
                Ok(_) => 1,
                Err(e) => {
                    eprintln!("failed to run brew: {e}");
                    1
                }
            };
        }
        InstallMethod::Nix => {
            eprintln!(
                "bushel lives in the read-only Nix store and can't update itself.\n\
                 Update the flake or channel that provides it, then rebuild."
            );
            return 1;
        }
        InstallMethod::Cargo | InstallMethod::Receipt => {}
    }

    let mut updater = axoupdater::AxoUpdater::new_for("bushel");
    if updater.load_receipt().is_err() {
        if method == InstallMethod::Cargo {
            eprintln!(
                "bushel was installed with cargo; upgrade with:\n  \
                 cargo install --git {} --force",
                env!("CARGO_PKG_REPOSITORY")
            );
        } else {
            eprintln!(
                "no install receipt found — bushel wasn't installed via the shell installer.\n\
                 Upgrade with whatever placed the binary, or reinstall with the installer\n\
                 from the latest GitHub release."
            );
        }
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
    match args.command {
        Some(Cmd::Update) => std::process::exit(self_update().await),
        Some(Cmd::Completions { shell }) => {
            completions::write_script(shell, &mut std::io::stdout())?;
            return Ok(());
        }
        None => {}
    }
    let cfg = Config::load();
    let no_splash = args.no_splash || cfg.no_splash;
    let reduced_motion = args.reduced_motion || cfg.reduced_motion;
    let ascii = args.ascii || cfg.ascii;

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
        engine.maybe_dissolve_splash();
        let elapsed = last_frame.elapsed();
        last_frame = Instant::now();
        if let Err(e) = terminal.draw(|f| ui.render(f, &engine.state, elapsed)) {
            break Err(e);
        }

        if engine.state.quit {
            break Ok(());
        }

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

        let frame_timeout = if ui.animating(&engine.state) {
            Duration::from_millis(33)
        } else if ui.ambient_active() {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(3600)
        };

        tokio::select! {
            _ = tokio::time::sleep(frame_timeout) => {}
            _ = poll.tick() => engine.on_tick(),
            ev = rx.recv() => {
                match ev {
                    Some(ev) => {
                        engine.apply(ev);
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
                        for cmd in keymap::map_key(&engine.state, k, &ui.last_info) {
                            engine.dispatch(cmd);
                        }
                    }
                    Some(Ok(_)) => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cargo_bins() -> Vec<PathBuf> {
        vec![PathBuf::from("/Users/x/.cargo/bin")]
    }

    #[test]
    fn brew_upgrade_forces_the_tap_refresh() {
        use std::ffi::OsStr;
        let cmd = brew_upgrade_command();
        assert_eq!(cmd.get_program(), OsStr::new("brew"));
        assert_eq!(
            cmd.get_args().collect::<Vec<_>>(),
            [OsStr::new("upgrade"), OsStr::new("bushel")]
        );
        let envs = cmd.get_envs().collect::<Vec<_>>();
        assert!(
            envs.contains(&(
                OsStr::new("HOMEBREW_AUTO_UPDATE_SECS"),
                Some(OsStr::new("0"))
            )),
            "the throttle has to be driven to zero: {envs:?}"
        );
        assert!(
            envs.contains(&(OsStr::new("HOMEBREW_NO_AUTO_UPDATE"), None)),
            "a shell-exported opt-out has to be dropped: {envs:?}"
        );
    }

    #[test]
    fn homebrew_cellar_wins() {
        assert_eq!(
            classify(
                Path::new("/opt/homebrew/Cellar/bushel/0.3.0/bin/bushel"),
                &cargo_bins()
            ),
            InstallMethod::Homebrew
        );
    }

    #[test]
    fn nix_store_is_nix() {
        assert_eq!(
            classify(
                Path::new("/nix/store/abc123-bushel-0.3.0/bin/bushel"),
                &cargo_bins()
            ),
            InstallMethod::Nix
        );
    }

    #[test]
    fn binary_in_cargo_bin_is_cargo() {
        assert_eq!(
            classify(Path::new("/Users/x/.cargo/bin/bushel"), &cargo_bins()),
            InstallMethod::Cargo
        );
    }

    #[test]
    fn nested_below_cargo_bin_is_not_cargo() {
        assert_eq!(
            classify(
                Path::new("/Users/x/.cargo/bin/nested/bushel"),
                &cargo_bins()
            ),
            InstallMethod::Receipt
        );
    }

    #[test]
    fn shell_installer_falls_through_to_receipt() {
        assert_eq!(
            classify(Path::new("/Users/x/.local/bin/bushel"), &cargo_bins()),
            InstallMethod::Receipt
        );
    }

    #[test]
    fn no_cargo_dirs_still_classifies() {
        assert_eq!(
            classify(Path::new("/Users/x/.cargo/bin/bushel"), &[]),
            InstallMethod::Receipt
        );
    }

    #[test]
    fn install_root_outranks_cargo_home() {
        let dirs = cargo_bin_candidates(
            Some(PathBuf::from("/opt/cargo-root")),
            Some(PathBuf::from("/Users/x/altcargo")),
            Some(PathBuf::from("/Users/x")),
        );
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("/opt/cargo-root/bin"),
                PathBuf::from("/Users/x/altcargo/bin"),
            ]
        );
        assert_eq!(
            classify(Path::new("/opt/cargo-root/bin/bushel"), &dirs),
            InstallMethod::Cargo
        );
    }

    #[test]
    fn cargo_home_outranks_home_dir() {
        let dirs = cargo_bin_candidates(
            None,
            Some(PathBuf::from("/Users/x/altcargo")),
            Some(PathBuf::from("/Users/x")),
        );
        assert_eq!(dirs, vec![PathBuf::from("/Users/x/altcargo/bin")]);
        assert_eq!(
            classify(Path::new("/Users/x/.cargo/bin/bushel"), &dirs),
            InstallMethod::Receipt
        );
    }

    #[test]
    fn home_dir_is_the_last_resort() {
        assert_eq!(
            cargo_bin_candidates(None, None, Some(PathBuf::from("/Users/x"))),
            vec![PathBuf::from("/Users/x/.cargo/bin")]
        );
        assert!(cargo_bin_candidates(None, None, None).is_empty());
    }
}

# bushel

A terminal UI for managing [Apple Containers](https://github.com/apple/container) — containers, images, and volumes from the comfort of your terminal. A bushel is a container that holds apples.

bushel is a lazydocker-style TUI built in Rust with [Ratatui](https://github.com/ratatui/ratatui). It wraps the `container` CLI as a subprocess and manages what already exists — it is a manager, not a launcher. Containers are born on the command line and managed in bushel.

## Requirements

- macOS 26 (Apple silicon)
- [`container`](https://github.com/apple/container) CLI **1.2.x** (bushel warns on untested versions but still runs)

## Install

Homebrew:

```sh
brew install frankieramirez/tap/bushel
```

Shell installer, no Homebrew required (installs to `$CARGO_HOME/bin`, default `~/.cargo/bin`):

```sh
curl -LsSf https://bushel.sh/install | sh
```

`https://bushel.sh/install` is a 302 to the installer attached to the latest GitHub
release. To skip the redirect, or to pin the hardened curl flags, fetch the asset
directly:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/frankieramirez/bushel/releases/latest/download/bushel-installer.sh | sh
```

`BUSHEL_INSTALL_DIR` picks a different prefix; `BUSHEL_NO_MODIFY_PATH=1` leaves your
shell rc files alone. Prebuilt archives and checksums are on the [releases page](../../releases).

From source — bushel isn't published to crates.io, so this needs `--git`:

```sh
cargo install --git https://github.com/frankieramirez/bushel
```

To upgrade, run `bushel update`. Shell-installer installs replace themselves in
place; Homebrew installs hand off to `brew upgrade bushel`; cargo, Nix, and
hand-placed binaries are told how to upgrade themselves instead.

## Use

```sh
bushel
```

- `1`/`2`/`3` or `Tab` — expand containers, images, or volumes (the others stay on the rail)
- `j`/`k`, `g`/`G` — move; `/` fuzzy filter; `Enter` focus the detail pane
- `space` — action menu for the selection (destructive actions tinted, always confirmed with the exact `container …` command about to run)
- `s` start/stop · `r` restart · `K` kill · `d` delete · `P` prune · `e` exec a shell
- `l`/`i` — Logs / Inspect detail tabs; `F` toggles log follow; `w` toggles wrap
- `m` — message log (full stderr of anything that failed) · `?` — help

Flags: `--no-splash`, `--reduced-motion`, `--ascii`. The same settings live in `~/.config/bushel/config.toml`:

```toml
no_splash = false
reduced_motion = false
ascii = false
```

## Design

The v0.1 scope, architecture, and rationale live in [SPEC.md](SPEC.md); the domain vocabulary in [CONTEXT.md](CONTEXT.md). The design was worked out in the open on the [wayfinder map](../../issues/1).

## License

[MIT](LICENSE)

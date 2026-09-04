# bushel

A terminal UI for managing [Apple Containers](https://github.com/apple/container) — containers, images, volumes, and networks from the comfort of your terminal. A bushel is a container that holds apples.

bushel is a lazydocker-style TUI built in Rust with [Ratatui](https://github.com/ratatui/ratatui). It wraps the `container` CLI as a subprocess and manages what already exists — it is a manager, not a launcher. Containers are born on the command line and managed in bushel.

## Requirements

- macOS 26 (Apple silicon)
- [`container`](https://github.com/apple/container) CLI **1.2.x** (bushel warns on untested versions but still runs)

## Install

Homebrew:

```sh
brew install frankieramirez/tap/bushel
```

Shell installer, no Homebrew required:

```sh
curl -LsSf https://bushel.sh/install | sh
```

From crates.io — a secondary path. Prefer Homebrew or the curl installer;
`bushel update` still follows GitHub Releases, not crates.io.

```sh
cargo install bushel
```

`https://bushel.sh/install` is a 302 to the checksummed installer attached to the
latest GitHub release; the domain never hosts a copy. To upgrade, run `bushel
update` — it works out how bushel was installed and hands the job back to
whatever did the installing.

The rest — `BUSHEL_INSTALL_DIR`, `BUSHEL_NO_MODIFY_PATH`, the hardened-curl
direct-asset form, pinning a version, and what `bushel update` does per install
method — is on **[bushel.sh/docs/install](https://bushel.sh/docs/install)**.

## Completions

```sh
bushel completions bash   # or zsh, fish
```

prints a script you can drop on your shell's completion path. That works for
Homebrew, the curl installer, and `cargo install`. GitHub releases also attach
the scripts and `bushel.1` so packagers can install them on the usual paths.
`man bushel` once that page is on your manpath. **[bushel.sh/docs](https://bushel.sh/docs)**
links those generated files instead of keeping a second copy of the man text.

## Use

```sh
bushel
```

Press `?` for the cheatsheet, `space` for the actions valid on whatever is
selected, and `,` for settings. Everything below is generated from bushel's own
source — the same list the help overlay draws — so it cannot drift from the
binary you are running.

bushel draws its body one of two ways, and `,` switches between them live:

- **rail** (the default) keeps all four panes in one column beside the detail
  pane, so you can see what else is running while you read one thing.
- **table** gives the active pane one full-width table and the detail below it,
  so logs get every column the terminal has and container rows carry state,
  uptime, memory ceiling, image, network, and volumes without truncating.

Whichever you pick is written to the config file, so it is there next launch.

<!-- keys:start -->

**global**

- `1/2/3/4, tab` — expand pane (containers / images / volumes / networks)
- `f` — zoom focused side
- `,` — settings (layout, glyphs, motion, splash)
- `m` — message log
- `b` — dismiss version banner
- `q` — quit

**list**

- `j/k g/G` — move / top / bottom
- `/` — fuzzy filter (esc clears)
- `enter` — focus detail pane
- `space` — action menu
- `s r K d P e` — start/stop · restart · kill · delete · prune · exec
- `u` — pull image (images pane)
- `t` — tag image (images pane)
- `c` — create volume (volumes pane)

**detail**

- `l / i` — logs / inspect tab (containers)
- `F` — toggle follow
- `w` — toggle wrap / truncated
- `pgup/pgdn` — scroll without switching focus
- `esc` — back to list
<!-- keys:end -->

Flags, and the `~/.config/bushel/config.toml` keys that set the same things:

<!-- options:start -->

- `--no-splash` / `no_splash = false` — Skip the splash screen
- `--reduced-motion` / `reduced_motion = false` — Disable all animation and effects
- `--ascii` / `ascii = false` — ASCII icons and spinners (no Unicode glyphs)
- `--layout` / `layout = "rail"` — Body layout: rail keeps all four panes in view, table gives one full-width table
<!-- options:end -->

A flag can only switch something on; it is ORed with the file. Full reference:
**[bushel.sh/docs/keys](https://bushel.sh/docs/keys)** and
**[bushel.sh/docs/config](https://bushel.sh/docs/config)**.

## Design

The docs live at **[bushel.sh/docs](https://bushel.sh/docs)** — install, keys,
config, troubleshooting, and [why bushel is shaped the way it
is](https://bushel.sh/docs/why). What's still to come is on the
[roadmap](../../issues/56).

For contributors: the v0.1 scope, architecture, and rationale are in
[SPEC.md](SPEC.md), the domain vocabulary in [CONTEXT.md](CONTEXT.md), and the
decisions that outlived their tickets in [docs/adr](docs/adr). The design was
worked out in the open on the [wayfinder map](../../issues/1).

## Getting help

- **Docs**: [bushel.sh/docs](https://bushel.sh/docs), including [troubleshooting](https://bushel.sh/docs/troubleshooting)
- **Questions**: [GitHub Discussions (Q&A)](https://github.com/frankieramirez/bushel/discussions/new?category=q-a)
- **Bugs / features**: [open an issue](https://github.com/frankieramirez/bushel/issues/new/choose) — see [CONTRIBUTING.md](CONTRIBUTING.md)
- **Security**: [SECURITY.md](SECURITY.md)

## License

[MIT](LICENSE) · [Code of Conduct](CODE_OF_CONDUCT.md)

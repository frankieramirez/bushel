# bushel

<p align="center">
  <img src="docs/assets/bushel-orchard-arcade.svg" alt="Bushel: pixel-art wooden lettering beside a basket of red apples" width="840">
</p>

Bushel is a terminal UI for managing [Apple Containers](https://github.com/apple/container)
on macOS. Browse containers, images, volumes, and networks, inspect logs, and run
actions from your keyboard.

Create containers with Apple's `container` CLI, then manage them in Bushel.
A bushel is a container that holds apples.

[Quick start](#quick-start) · [Controls](#everyday-controls) · [Settings](#settings-and-layouts) · [Docs](https://bushel.sh/docs) · [Help](#getting-help)

## Requirements

- macOS 26 on Apple silicon.
- Apple's [`container` CLI](https://github.com/apple/container) installed and available on your `PATH`.

Bushel tests against `container` **1.2.x**. The current compatibility guidance
recommends **1.3.1** for Apple's security fixes; Bushel allows it with a dismissible
untested-version warning while compatibility validation finishes.

## Quick start

Install Bushel with Homebrew, then launch it:

```sh
brew install frankieramirez/tap/bushel
bushel
```

If Bushel reports that the container service is down, press `s` to start it.
The first start can take longer while Apple installs the Linux kernel.

Bushel lists the resources that already exist on your machine. If you haven't
created any containers yet, follow Apple's
[container guide](https://github.com/apple/container) to create one from the CLI.

<details>
<summary>Other installation methods</summary>

**Shell installer**

```sh
curl -LsSf https://bushel.sh/install | sh
```

**Cargo**

```sh
cargo install bushel
```

Homebrew and the shell installer are the preferred installation methods.
Cargo installs are also supported, but `bushel update` follows GitHub Releases.

See the [installation guide](https://bushel.sh/docs/install) for custom paths,
pinned versions, and installer verification.

</details>

## Everyday controls

| Key | Action |
| --- | --- |
| `j` / `k` | Move through a list |
| `tab` | Switch panes |
| `enter` | Focus the selected item's details |
| `space` | Open the actions available for the selection |
| `/` | Filter the list |
| `?` | Open the cheatsheet |
| `m` | Read the message log |
| `q` | Quit |

<details>
<summary>All keyboard shortcuts</summary>

This reference comes from Bushel's source. Use `?` for the cheatsheet that matches
your installed version.

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

</details>

The [keyboard reference](https://bushel.sh/docs/keys) is also available online.

## Settings and layouts

Press `,` to change settings. Bushel saves your preferences in
`~/.config/bushel/config.toml` for the next launch.

| Layout | What you see |
| --- | --- |
| **rail** (default) | All four resource panes in a column beside the detail pane |
| **table** | The active resource list across the window, with details below |

To try the table layout for one session:

```sh
bushel --layout table
```

Command-line flags apply to the current run. `--layout` overrides the saved
layout; boolean flags enable an option even when the config sets it to `false`.
To enable one permanently, set its config value to `true`.

<details>
<summary>Command-line options and config defaults</summary>

Each row lists a flag and its config key with the **default value**. For example,
`no_splash = false` keeps the splash screen enabled; `--no-splash` skips it.

<!-- options:start -->

- `--no-splash` / `no_splash = false` — Skip the splash screen
- `--reduced-motion` / `reduced_motion = false` — Disable all animation and effects
- `--ascii` / `ascii = false` — ASCII icons and spinners (no Unicode glyphs)
- `--layout` / `layout = "rail"` — Body layout: rail keeps all four panes in view, table gives one full-width table
<!-- options:end -->

</details>

See the [configuration reference](https://bushel.sh/docs/config) for details.

## Updating

```sh
bushel update
```

Bushel detects how you installed it and selects the update method. See the
[installation guide](https://bushel.sh/docs/install) for behavior by install method.

<details>
<summary>Shell completions and manual page</summary>

Generate completions for your shell:

```sh
bushel completions bash   # or zsh, fish
```

Save the output in your shell's completion directory. GitHub releases also
include completion scripts and `bushel.1`. Once the manual page is on your
manpath, read it with `man bushel`.

The [documentation index](https://bushel.sh/docs) links to these files.

</details>

## Documentation and contributing

The [user guide](https://bushel.sh/docs) covers everyday use and troubleshooting.
Read [why Bushel works this way](https://bushel.sh/docs/why) for the design
rationale, or follow the [roadmap](../../issues/56) for planned work.

Bushel uses Rust and [Ratatui](https://github.com/ratatui/ratatui), and runs Apple's
`container` CLI as a subprocess. Start with [CONTRIBUTING.md](CONTRIBUTING.md)
before proposing a change.

| Contributor reference | Contents |
| --- | --- |
| [SPEC.md](SPEC.md) | Original scope and architecture |
| [CONTEXT.md](CONTEXT.md) | Project vocabulary |
| [Decision records](docs/adr) | Design decisions and their rationale |
| [Planning map](../../issues/1) | Original design discussion |

## Getting help

If an action fails, press `m` to read the command output in Bushel's message log.
Include your Bushel version, `container --version`, macOS version, and the
relevant log output when reporting a bug.

- **Troubleshooting:** [Common problems and fixes](https://bushel.sh/docs/troubleshooting)
- **Questions:** [GitHub Discussions](https://github.com/frankieramirez/bushel/discussions/new?category=q-a)
- **Bugs and feature requests:** [Open an issue](https://github.com/frankieramirez/bushel/issues/new/choose)
- **Security reports:** Follow [SECURITY.md](SECURITY.md)

## License

[MIT](LICENSE) · [Code of Conduct](CODE_OF_CONDUCT.md)

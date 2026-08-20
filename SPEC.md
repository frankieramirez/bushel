# bushel v0.1 — specification

A lazydocker-style TUI for Apple Containers. bushel wraps the `container` CLI (1.2.0+, macOS 26) as a subprocess and manages what already exists — it is a manager, not a launcher. It never links Apple's Containerization framework.

This spec is the output of the [wayfinder map](https://github.com/frankieramirez/bushel/issues/1); each section links the decision ticket that holds its full rationale. The domain vocabulary used throughout is defined in [CONTEXT.md](CONTEXT.md). Build sessions should be able to execute from this document without fundamental questions.

## Positioning

Built primarily for the author's own workflow, published openly with proper packaging. Prior art ([dissected here](https://github.com/frankieramirez/bushel/issues/2)) is a source of ideas and a bar to clear: the popular TUI in this niche is read-only and parses table output; the deep one is GPL. bushel's slot: a focused, MIT-licensed containers/images/volumes TUI with lifecycle actions, command-preview safety, and first-class polish.

**Stack**: Rust + Ratatui + tachyonfx, tokio for async subprocess work. Single binary, macOS-only.

## Scope

Three entities, one pane each: **containers** (primary), **images**, **volumes**.

**Non-goals for v0.1** (settled at charting and in the [feature cut](https://github.com/frankieramirez/bushel/issues/4)):

- No build UI, registry login/management, machine management, or networks pane
- No container creation from the TUI (no `run`/`create` dialog) — containers are born on the command line, managed in bushel
- No multi-select or bulk marking (per-entity prune covers bulk cleanup)
- No `system stop` exposure (kill-all-containers footgun)
- No image push/tag/save/load/build; no volume create
- No compose emulation

## Feature cut

Full detail: [v0.1 feature cut](https://github.com/frankieramirez/bushel/issues/4).

### Containers (primary pane)

- **List**: all containers, running + stopped; running-first then alphabetical. Columns include lightweight **CPU% / mem**, sampled per poll tick from `stats --no-stream --format json`, CPU% derived client-side from consecutive cumulative samples.
- **Inspect**: scrollable pretty-printed JSON.
- **Logs**: bounded tail (~last 1000 lines) with **follow** toggle. bushel owns and kills its `logs -f` subprocesses (they never exit on their own when a container stops).
- **Lifecycle**: start, stop, kill (default signal, no picker), delete, prune, plus **synthetic restart** (stop → start; the CLI has no restart subcommand).
- **Exec**: suspend the TUI, `container exec -it <id> /bin/sh` inheriting stdio (single attempt, no bash fallback in v0.1), restore on exit.

### Images

- List, inspect, delete, prune.
- **Pull** via one-line reference prompt (tag defaults to `latest`); the CLI's own progress output surfaced raw, streamed in the detail pane — never a blocking modal.

### Volumes

- List, inspect, delete, prune.
- **In-use badge** on volumes referenced by containers; delete of an in-use volume is blocked with an error.

### Cross-cutting

- **Command-preview safety**: every confirmable action shows the exact `container …` command about to run. Confirm set: **delete, prune, kill**. Start/stop/restart/pull run without confirmation; reads never prompt.
- **Service**: startup probes `system status`; when down, a full-screen takeover offers one-key `container system start --enable-kernel-install` (the bare command blocks on an interactive kernel prompt — [CLI-surface research](https://github.com/frankieramirez/bushel/issues/3)).

## UX skeleton

Full detail: [UX skeleton](https://github.com/frankieramirez/bushel/issues/5). Validated end-to-end by the [layout prototype](https://github.com/frankieramirez/bushel/issues/7) with **no changes** — the prototype branch [`prototype/layout-mock`](https://github.com/frankieramirez/bushel/tree/prototype/layout-mock/prototype-layout) is the living reference for look and feel.

### Layout

- Hybrid lazydocker split: persistent left entity list (~45%) + right **detail pane** (~55%); `f` zooms the focused pane fullscreen.
- One pane at a time: `1/2/3` jump to containers/images/volumes, `Tab` cycles.
- Detail tabs: containers get `Logs | Inspect` (Logs default, `l`/`i` jump); images and volumes have Inspect only.
- Focus: `Enter` moves focus into the detail pane, `Esc` returns; PgUp/PgDn scroll the detail pane without switching focus.

### Input

- Vim-flavored (`j/k`, `g/G`) plus arrows; `/` fuzzy filter over name/image/status, `Esc` clears.
- Direct action keys everywhere, plus the **`space` action menu**: a bottom sheet listing valid actions for the selection with their keys, destructive ones tinted. Doubles as key discovery.
- `?` help overlay: complete grouped cheatsheet. The bottom bar is the primary discovery path.

### Confirmations, errors, resilience

- Destructive confirm: centered modal, the exact command as its body, `y` runs / `Esc` cancels.
- Errors: status-bar one-liner with the stderr gist; a **message log** scrollback (`m`) holds full stderr. No modal-per-error.
- Service down: full-screen takeover with one-key start, output streamed. Version mismatch: dismissible banner, never blocking.
- Bottom bar: context-sensitive key hints + right-aligned status cluster (service dot, CLI version, poll spinner).
- Pull: modal input; progress streams in the detail pane with a status-bar spinner.
- Pending actions show a spinner on the entity's row until a poll confirms the outcome.

### Motion language ([ADR 0001](docs/adr/0001-motion-first-tui.md), prototype-confirmed)

- **Splash-as-loading**: animated bushel mark plays only while startup probes run, any key skips, dissolves into the layout. `--no-splash` flag + config disable.
- **Micro-motion baseline**: pane-switch sweep, modal fade-in, bottom-sheet slide, focus transitions, toast slide-in. Hard rules: **≤150ms, interruptible, never delays data or input**; `reduced-motion` config kills all of it.
- **Ambient effect**: the wordmark hue drift passed its prototype gate (**kept** — reads as polish, ~1.3ms worst frame). Still disabled by `reduced-motion`.
- **Aesthetic**: dark-first, gradient accents, rounded borders, Nerd Font icons with ASCII fallback, truecolor → 256-color auto-detect. Palette starting point (validated in the prototype): `#0f1117` ground, orchard-green `#7ee787` → apple-red `#ff7b72` gradient accents. Final mark/GIFs are build-phase work.

## Architecture

Full detail: [wrapper architecture](https://github.com/frankieramirez/bushel/issues/6).

### Layers

`runner` → `client` → `engine` → `ui`:

- **Runner**: the low-level subprocess seam — `run(args) -> Output` and `spawn_stream(args) -> (LineStream, KillHandle)`. Mocked at the args→bytes level. Exec and `logs -f` also route through it.
- **Client**: arg-building, serde parsing (no `deny_unknown_fields` — JSON shapes are only patch-stable), error classification. All version fragility is confined here, tested against per-version fixtures.
- **Engine**: owns `AppState`, the poller, the action queue, the log follower. Headlessly testable; knows nothing about rendering.
- **UI**: Ratatui + tachyonfx; consumes `AppState`, emits commands, never touches the Client.

### Refresh model

- No event API exists, so poll: fixed **1s tick**; containers re-listed every tick; images/volumes on pane entry, after mutating actions, and every ~10th tick. `stats --no-stream` piggybacks only while running containers exist.
- Service down (`XPC connection error` classification): entity polling stops; probe `system status` every 2s until recovery.
- JSON parse failure: keep last good state, log; degraded banner only after 3 consecutive failures.

### Process model

- Elm-style loop: one `AppState`, one `AppEvent` enum; all tokio tasks (poller, action runners, log follower) are pure producers into one mpsc channel; single-writer update loop.
- Render on event, plus a ~30fps frame ticker armed **only** while an effect or splash is active. Idle CPU near zero.
- At most one pending action per entity id; polls always overlap actions; synthetic restart is one pending action. Pending clears when a poll confirms the expected state (capped at 2 ticks) or immediately on failure.
- Log follower lives only while the Logs tab shows a running container: `logs -n 200` backlog then `logs -f`; killed on selection/tab change or when a poll shows the container stopped; ~10k-line ring buffer.
- Exec: pause poller, kill follower, restore terminal, run inheriting stdio, re-enter alt-screen, full redraw, immediate poll.
- Timeouts: 10s on reads; none on mutating actions. `pull`/`prune` show pending + elapsed, stderr lines feed the message log.

### Errors

`CliError`: `ServiceDown`, `NotFound`, `InUse`, `Usage` (exit 64 = bushel bug), `ParseFailure`, `Timeout`, `Other(String)` — classified by exit code first, stderr substring second; raw stderr always preserved in the message log. `NotFound` during an action is a status-bar notice only.

### External changes & version checks

- Poll diff announces only terminations not caused by bushel ("stopped externally"); other external changes update silently. All diffs land in the message log.
- Startup `container --version` checked against a hard-coded tested range kept next to the fixtures; outside it → dismissible banner naming the range. Never refuses to start.

## Testing strategy

Fixture-based tests at the Client layer (per-version captured JSON through the real parsers) + headless Engine tests (inject events, assert state) + a mock Runner replaying fixtures for end-to-end engine runs. The UI stays thin; it is covered by the prototype and manual passes.

## Packaging & distribution

Settled at spec assembly:

- **License**: MIT.
- **Releases**: [cargo-dist](https://opensource.axo.dev/cargo-dist/) — generates the GitHub Actions release workflow, builds macOS binaries on tags, and maintains the Homebrew tap formula (`frankieramirez/homebrew-tap`).
- **Versioning**: semver from `0.1.0`; tags `vX.Y.Z` trigger releases.
- **CI**: GitHub Actions on PRs and main — `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- **Repo hygiene**: standard OSS from day one — LICENSE, CONTRIBUTING.md, bug/feature issue templates (in this PR).
- **README**: gets install instructions and demo GIFs during the build phase (branding work).

## Build-phase pointers

- The prototype branch [`prototype/layout-mock`](https://github.com/frankieramirez/bushel/tree/prototype/layout-mock/prototype-layout) is throwaway — steal its look and feel, not its code. It never merges.
- Capture real `container` 1.2.x JSON fixtures early; they anchor the Client tests and the tested-version constant.
- Known CLI traps to honor from day one ([research](https://github.com/frankieramirez/bushel/issues/3)): `logs -f` never exits on container stop; stopped containers record no exit code; bare `system start` blocks on an interactive prompt; JSON shapes are patch-stable only.

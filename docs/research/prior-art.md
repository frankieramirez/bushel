# Prior art: Apple Containers management tools

Research for [issue #2](https://github.com/frankieramirez/quayside/issues/2), part of map [#1](https://github.com/frankieramirez/quayside/issues/1).
Researched 2026-08-19 against the GitHub repos (READMEs, source, issue trackers — not cloned) and the web.

Context: Apple's [`container`](https://github.com/apple/container) CLI (WWDC 2025, Swift, Apple silicon only) has no official UI. A small ecosystem of TUIs and GUIs has grown around it in 2025–2026. Apple's own tracker has a ["TUI for the container CLI" discussion (#1249)](https://github.com/apple/container/discussions/1249) where people explicitly ask for a k9s/lazygit-style experience.

## TUIs

### andreybleme/lazycontainer — 370★, Go + Bubble Tea, MIT

The incumbent by stars, and the closest analog to quayside. Homebrew core formula (`brew install lazycontainer`). Active (last push 2026-08-17).

- **Feature coverage**: read-only. Lists containers and images, inspects both. Logs support just merged (PR #16). No volumes, no networks, no exec, no stats, no build, no lifecycle actions (start/stop/delete are all missing — open issue #4 "Support container actions").
- **UX model**: two-pane list + details. `tab` switches containers ↔ images, arrows navigate, `enter` inspects, `q` quits. No command mode, no help screen, minimal keymap.
- **Stack**: Go, Bubble Tea, tiny codebase (`cmd/main.go` + `pkg/container`, `pkg/image`, `pkg/util` — five source files).
- **Integration**: shells out to `container` via `os/exec` and **parses the human-readable table output with `strings.Fields`** (`container list --all`, positional columns ID/IMAGE/OS/ARCH/STATE), falling back to JSON only for `inspect`. Fragile: any column change in the CLI breaks it. Pinned to container CLI 0.1.0 in the README while the CLI is at 1.x.
- **Gaps / pain points from the tracker**: open issues ask for stats (#15), networks (#6), container actions (#4), better details formatting (#3). The project is self-described alpha; it earns its stars from being first and brew-installable, not from depth.

### pzep1/lazycontainer — 15★, Go + Bubble Tea, GPL-3.0

A different, far more ambitious project with the same name. v0.5.2, self-tapped Homebrew formula. This is the feature ceiling of the space.

- **Feature coverage**: essentially everything the CLI exposes — containers (start/stop/restart/kill/delete/exec/one-off commands/copy files/export tar), images (pull/build/tag/push/save/load/run/history), volumes, networks, machines, registries, builder, system diagnostics, VM boot logs, local DNS domains. Live log streaming with autoscroll, live CPU/mem/net/disk ASCII graphs (CPU% derived from Apple's cumulative `cpuUsageUsec`), bulk actions/prune menus, in-use badges (●N containers referencing an image/volume/network), and a **compose emulation layer**: it reads a `compose.yaml` and orchestrates it via `container run`/`stop`/`delete` in dependency order, since Apple's CLI has no compose.
- **UX model**: lazydocker-faithful. All resource panels stacked in a left sidebar, all visible at once (no accordion); focused panel gets accent bar + most vertical space; right main panel with tabs (Config · Logs · Stats · Env · Ports · Mounts · Health · Top · Inspect) cycled with `[`/`]`. `tab`/`1`–`9` for panel focus, `space` context-aware actions menu, `B` bulk actions, `/` filter, `:`/`;` ad-hoc and named custom commands, `?` scrollable keybinding help, `+`/`_` screen modes, mouse support. JSON config with themes, custom per-context commands with `{container}`-style placeholders, live config reload.
- **Stack**: Go 1.26, Bubble Tea, well-factored (`internal/containercli`, `internal/compose`, `internal/tui`, `internal/config`) with tests throughout, CI, release workflow that bumps the brew formula.
- **Integration**: shells out, but properly — a `Runner` interface over `exec.CommandContext` with per-call timeouts (15s default, 30m for long ops), and **every list/inspect call uses `--format json`** (`list --all --format json`, `image list --format json --verbose`, `volume list --format json`, …). The Runner interface makes the whole client mockable for tests.
- **Gaps / pain points**: zero issues filed — all five closed PRs are the author's own. It's a one-person sprint with no community; GPL-3.0 license; the kitchen-sink scope (9 panels, machines, registries, compose) makes for a dense, busy first screen. Discoverability is compensated by menus, but the surface is large.

### matteobisi/apple-container-tui (`actui`) — 9★, Go + Bubble Tea, MIT

A proof-of-concept for the author's spec-kit AI-assisted development workflow; the tool is secondary to the process demo.

- **Feature coverage**: containers (list/start/stop/delete/logs/shell/export), images (list/pull/build/prune/inspect/delete), machines (full 1.0 machine management), registries view, daemon start/stop. No volumes, no networks, no stats.
- **UX model**: menu-driven rather than panel-driven — a container list where `enter` opens an action submenu, and single keys jump to whole other screens (`i` images, `M` machines, `m` daemon, `g` registries). Distinctive safety UX: **command preview before every execution**, type-to-confirm deletes, a `--dry-run` mode, and JSONL command logs with rotation. TOML config, theme auto mode.
- **Stack**: Go 1.24+, Bubble Tea, unusual `src/models` + `src/services` layout with per-command builder services (`command_builder.go`, `container_exec_builder.go`, …), tests, CI with SBOM/provenance releases.
- **Integration**: builds `container …` argv via builder services, executes as subprocess; structured daemon status (running/stopped/unknown).
- **Gaps / pain points**: zero issues; no volumes/networks; screen-per-resource navigation means lots of `esc`-ing back; requires macOS 26 + container 1.0+.

### kGeee/container-tui — 0★, Go + Bubble Tea + Lip Gloss, Apache-2.0

The community starter that came out of apple/container discussion #1249 (originally saehejkang/container-tui, now 404; this copy is not a fork flag-wise but is the same lineage). Only `system start/stop/status` implemented; a generic `RunCommand` wrapper over `exec.Command`. Dormant since 2026-02. Evidence of demand, not a competitor.

## GUIs

### andrew-waters/orchard — 897★, Swift/SwiftUI, MIT

The dominant GUI. `brew install orchard` (homebrew cask). Differentiates on **local AI**: wires MLX/Ollama/LM Studio model servers into containers (injects `OPENAI_BASE_URL`, computes container-reachable endpoints) and offers "Sandboxes" — agent containers behind the hypervisor boundary with a kill switch. Also full machine management, live CPU/mem/net/disk charts, multi-pane log viewer.

- **Integration**: the deep end — drives machines over Apple's **native XPC API** (`MachineAPIClient`) and links the Swift packages rather than only shelling out.
- **Pain points from its 70+ issue tracker**: the tight coupling bites — recurring breakage across container CLI releases ("buttons not working after v1.7.0" #17/#54, "launched containers not detected" #39, "list not reflecting actual running state" #38, apiserver health-check decode errors #37, crashes #20, an issue on whether to lock Orchard to the container version #19). Also PATH detection (#35), exec assuming shell paths (#44/#64), edit-config losing fields (#42). The lesson: tracking private Swift APIs gives power and costs stability at every upstream release.

### sembsa/ContainerDesktop — 71★, Swift/SwiftUI, deliberately CLI-only

Docker Desktop-style. Explicit design statement: *"does not reimplement any container logic: every action shells out to the official `container` CLI and parses its JSON output."* Very complete: containers (logs with severity colouring, live stats via Swift Charts, file browser, embedded SwiftTerm terminal), images/volumes/networks/registries/machines, a rich run-container dialog with a **live copy-pasteable shell preview of the exact command**, compose translation (with `/etc/hosts` wiring to work around broken DNS in container 1.0.0, `x-init` one-off tasks), experimental k8s/Helm via the `container k8s` plugin, a WidgetKit widget. Nearly empty issue tracker.

### ContainerUI (container-ui.fly.dev) — Swift, closed distribution

Native Mac app; ⌘K command palette; Docker Hub search; shell-in-Terminal/iTerm2 handoff. Publishes the same architectural argument: it *"shells out to the supported container CLI and decodes its `--format json` output"* because the Swift packages *"lack API stability beyond patch versions"* — views → view models → services → `ContainerCLI`/`CommandRunner`/`Process`, testable with mock runners.

### Others

- **ducheharsh/apple-container-desktop** — 74★, React + Tauri. Every open issue is an environment failure ("stuck on verifying api server", "app is damaged and can't be opened", broken images tab). Unmaintained since 2025-10. Cautionary tale for non-native wrappers.
- **Podman Desktop Apple Container extension** — official extension; list containers/images and view logs inside Podman Desktop. Read-mostly.
- Smaller ones circulating: AppleContainerGUI, iContainer (per Wikipedia's Apple container page).

## Comparison

| | andreybleme/lazycontainer | pzep1/lazycontainer | actui | Orchard | ContainerDesktop |
|---|---|---|---|---|---|
| Stars | 370 | 15 | 9 | 897 | 71 |
| Type / stack | TUI, Go+Bubble Tea | TUI, Go+Bubble Tea | TUI, Go+Bubble Tea | GUI, SwiftUI | GUI, SwiftUI |
| Containers | list/inspect/logs | full lifecycle + exec + copy/export | lifecycle + logs/shell/export | full + terminal | full + terminal + file browser |
| Images | list/inspect | pull/build/tag/push/save/load/run | pull/build/prune/delete | yes | pull/build/run |
| Volumes | — | yes (+ in-use badges) | — | yes | yes (+ file browser) |
| Logs | just added | live streaming, autoscroll | view | multi-pane | severity colouring |
| Exec | — | yes | shell | terminal | SwiftTerm embedded |
| Stats | — (open issue) | live ASCII graphs | — | live charts | Swift Charts, 1s refresh |
| Build | — | yes | yes | — | yes |
| CLI integration | subprocess, **parses table text** | subprocess, `--format json`, Runner iface, timeouts | subprocess via command builders | **XPC + Swift packages** | subprocess, `--format json` |
| Safety UX | none | confirm keys on destructive | command preview, type-to-confirm, dry-run | guardrails | command preview in run dialog |
| Health | active, shallow | deep, no community | PoC | active, breakage-prone | active |

## Implications for quayside

**Steal**

1. **`--format json` everywhere, behind a Runner interface** (pzep1's `internal/containercli` is the reference): `exec.CommandContext` with per-call timeouts (short for lists, long for pull/build), `CombinedOutput` folded into errors, and a mockable `Runner` so the whole CLI client is testable without a Mac in CI. Never parse table output — that is andreybleme's structural weakness and the first thing an upstream column change breaks.
2. **The CLI-subprocess stance as an explicit design principle, stated in the README** the way ContainerDesktop and ContainerUI do. Orchard's issue tracker is the proof: XPC/Swift-package coupling breaks on nearly every container release (#17, #37, #38, #39, #54). The CLI is Apple's only stable, documented surface.
3. **lazydocker panel grammar** (pzep1): resource panels on the left, tabbed detail panel on the right, `tab`/number keys to move focus, `[`/`]` for detail tabs, `/` filter, `?` help. It's the layout every user of lazygit/lazydocker/k9s already knows — the discussion in apple/container #1249 asks for exactly this.
4. **Discoverability over memorization**: a context-aware actions menu on `space` (pzep1) so nobody needs the keymap to act, plus a scrollable `?` reference.
5. **Safety UX from actui**: confirmation on destructive actions and — cheap to add, high trust payoff — showing the exact `container …` command before/while running it. ContainerDesktop's copy-pasteable command preview doubles as CLI education and debuggability.
6. **In-use badges** (pzep1) for images and volumes ("●3 containers use this") — small feature, directly answers "is this safe to delete/prune", and fits the v0.1 resource set exactly.
7. **CLI-version handling**: detect `container system version` at startup and warn on untested majors (Orchard #19/#21 show upstream moves fast; andreybleme is still pinned to 0.1.0).

**Avoid**

1. **Table-output parsing** and pinning to an old CLI (andreybleme).
2. **XPC / Swift-package coupling** (Orchard's whole bug tail). Not even reachable from Go, but worth recording as a decision.
3. **Kitchen-sink v0.1** (pzep1 has nine panels, machines, registries, compose, custom command DSLs — and 15 stars). Depth on containers/images/volumes beats breadth; ship the three panels and make them excellent.
4. **Compose emulation** for now. Both pzep1 and ContainerDesktop hand-roll orchestration over `container run` with dependency ordering and DNS workarounds — it's the largest, most fragile subsystem in each. Fine as a later differentiator, wrong as early scope.
5. **Screen-per-resource navigation with esc-stacks** (actui). Persistent panels beat modal screens for a monitoring-style tool.
6. **Non-native wrapper stacks** (ducheharsh's Tauri app: every open issue is packaging/environment). Go single-binary + brew tap avoids the whole class.

**Add (gaps nobody in the TUI space fills well)**

1. **A polished 370-star-shaped hole**: the popular TUI is read-only; the complete TUI is unknown and GPL. A focused MIT Go+Bubble Tea TUI with full lifecycle on containers/images/volumes, done well, has no direct occupant.
2. **Stats in the TUI without the sprawl**: andreybleme's most-wanted open issue (#15) is stats; ContainerDesktop/Orchard show the GUI bar. A lightweight CPU/mem column in the container list (from `container stats`) covers most of the want at a fraction of pzep1's graph machinery.
3. **First-run and error empathy**: every tool's tracker has "service not running / CLI not found / PATH" issues (Orchard #34/#35, ducheharsh #6). Detect `container` absence and a stopped system service at launch and offer the fix (`container system start`) as a one-key action instead of an error string.
4. **Volumes as a first-class v0.1 panel**: of the four TUIs only pzep1 has volumes at all. Shipping containers + images + volumes at v0.1 already exceeds the incumbent's coverage.

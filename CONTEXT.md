# bushel

A lazydocker-style TUI for Apple Containers. bushel wraps the `container` CLI as a subprocess and manages what already exists — it is a manager, not a launcher.

## Language

### Entities

**Container**:
A container known to the Apple Containers service, running or stopped. bushel's primary entity.

**Image**:
A locally-present OCI image, pullable by reference.

**Volume**:
A named storage volume. A volume is **in use** when at least one container references it. bushel can create one by name only, and delete it; advanced create flags stay on the CLI.
_Avoid_: mount

**Network**:
A network known to the Apple Containers service. Read-only in bushel v1.

### Interaction

**Pane**:
One of the four entity views (containers, images, volumes, networks). All four are present in the rail; one is the active panel.
_Avoid_: tab, screen, page

**Layout**:
Which of two shapes the body takes, chosen in the settings panel and stored as `layout` in the config file. **Rail** is the default. **Table** is the alternative. The ladder, the floor, zoom, focus, and every key are the same in both; only the body differs.
_Avoid_: mode (Table makes the *pane* a mode; the layout is not one), theme, view

**Rail** (the layout):
All four panes in one borderless column beside the detail pane, separated from it by a single rule. Four typographic sections, no boxes. Each section shrinks to fit its rows and the leftover pools once, at the bottom of the rail, above the footer.
_Avoid_: sidebar, stack

**Table** (the layout):
One full-width table above one full-width detail pane. The resource type becomes a mode: only the active pane's rows are on screen, and the header carries the counts for the other three. Spare width buys columns, not padding.
_Avoid_: grid, list view

**Section**:
One pane's block in the rail: a label row carrying the pane's name and count, then its rows. What used to be a bordered box.
_Avoid_: box, card

**Rail footer**:
The rail's last row: reclaimable image bytes and the key that frees them.

**Selection bar**:
The `▎` in the first column of the selected row. Accent when the list has focus, dim when the detail pane does. It replaces the highlighted rectangle in the rail; the table keeps a highlight behind it.

**Active panel**:
The pane in the rail that currently receives list input and owns the detail pane's selection.
_Avoid_: focused pane (Focus is list vs detail)

**Collapse**:
The shrinking of an inactive rail pane: shrink-to-fit names when roomy, a 1-row title+count when tight.

**Tier**:
A layout size band. The ladder has two placements: stacked (body width under 80) and beside (otherwise). Below that sits the **floor**: a frame of 22 rows or fewer, or 60 columns or fewer, where chrome is shed. Nothing refuses to draw at any size.

**Detail pane**:
The view of the current selection. Beside the rail on medium and wide terminals; below it when stacked. For containers it has two detail tabs (Logs, Inspect) and the strip; images, volumes, and networks have Inspect only.

**Strip**:
The persistent telemetry view at the top of the detail pane for the selected container: cpu and memory sparks, plus network and disk rates. A short ring buffer (~300 samples at the poll rate) backs the sparks; display is clipped to spark width. Rates stay instantaneous. Three shapes by width: one row in the table layout, two beside the rail, three when neither fits.
_Avoid_: dashboard, graph (the sparkline is the glyph, not the view), configurable history, overview

**Focus**:
Which side of the layout receives input — the rail's active panel or the detail pane. Enter moves focus into the detail pane; Esc returns it to the list.

**Zoom**:
Toggling the focused side to fullscreen and back. Zoom fullscreens the active panel's table, not the whole rail.

**Filter**:
Fuzzy narrowing of the active panel's list; Esc clears it. Inactive rail panes stay unfiltered.
_Avoid_: search

**Action menu**:
The bottom sheet opened with `space`, listing the valid actions for the current selection alongside their direct keys.
_Avoid_: context menu, palette

**Bottom bar**:
The persistent bar of context-sensitive key hints on the left and, on the right, the actions valid on the current selection. A toast or a running activity takes the whole bar while it lasts.
_Avoid_: status bar (ambiguous with the status cluster)

**Status cluster**:
Service state, CLI version, and the poll spinner, right-aligned in the header. Dropped at the floor, where the row is not long enough to hold it.

**Settings panel**:
The overlay on `,` listing every field of the config file — layout, ascii glyphs, reduced motion, splash — with its current value. Toggling a row takes effect at once and writes `~/.config/bushel/config.toml`.
_Avoid_: preferences, options dialog

**Message log**:
The scrollback of recent errors and notices, holding the full stderr behind each one-line gist on the bottom bar. Every toast writes through it, so it is a superset of what the bottom bar showed. Last 1,000 entries.
_Avoid_: console

**Splash**:
The animated launch screen that plays only while the startup probes run; any key skips it, and it never adds latency of its own — except on the very first launch on a machine, where it dwells for up to a second so the mark is seen once. `--no-splash` and `--reduced-motion` skip that too.
_Avoid_: intro, loading screen

**Action**:
A keybound operation on the selected entity.

**Destructive action**:
An action that requires confirmation before running: delete, prune, and kill.
_Avoid_: dangerous action

**Command preview**:
The exact `container …` command shown in a destructive action's confirmation step before it runs.

**Restart**:
The synthetic stop-then-start action. The `container` CLI has no restart subcommand; bushel composes it.

**Follow**:
The live-tailing mode of the logs view, backed by a `logs -f` subprocess that bushel owns and kills.
_Avoid_: stream, watch

**Wrap**:
The logs-view mode in which a raw log line occupies as many display rows as the pane width requires. On by default. Opposite of truncated.
_Avoid_: word wrap, soft wrap

**Exec**:
Suspending the TUI and attaching the terminal to a shell inside the selected container, restoring the TUI on exit.
_Avoid_: attach, shell into

**Prune**:
The CLI's own bulk-cleanup action per entity (stopped containers, unused images, unreferenced volumes). bushel's only bulk operation.

**Tag**:
The images-pane action that assigns a new local reference to an existing image.
_Avoid_: push

**Pending**:
The state of an entity whose action is in flight. An entity can have at most one pending action; pending clears when a poll tick confirms the outcome.
_Avoid_: busy, locked

**External stop**:
A container leaving the running state without a bushel action. The only external change bushel announces; all others update the lists silently.

### Engine

**Runner**:
The mockable seam through which every `container` CLI invocation passes.
_Avoid_: executor

**Client**:
The typed layer above the Runner that builds commands, parses their output into entities, and classifies their errors. All version fragility lives here.

**Engine**:
The headless core that owns application state, the poll loop, pending actions, and the follow subprocess. It knows nothing about rendering.

**Poll tick**:
One iteration of the periodic refresh loop that re-lists entities and samples stats. Apple Containers has no event API, so polling is the only refresh model.

**Service**:
The Apple Containers system service (`container system …`) that daemon-backed commands require. bushel probes it at startup and offers a one-key start when it is down.
_Avoid_: daemon

# bushel

A lazydocker-style TUI for Apple Containers. bushel wraps the `container` CLI as a subprocess and manages what already exists — it is a manager, not a launcher.

## Language

### Entities

**Container**:
A container known to the Apple Containers service, running or stopped. bushel's primary entity.

**Image**:
A locally-present OCI image, pullable by reference.

**Volume**:
A named storage volume. A volume is **in use** when at least one container references it.
_Avoid_: mount

### Interaction

**Pane**:
One of the three entity views (containers, images, volumes). All three are present in the rail; one is the active panel.
_Avoid_: tab, screen, page

**Rail**:
The always-present column of all three panes. Inactive panes collapse; the active panel takes the flexible space. Below three rows there is not one row per pane to give, and the active panel takes what the rail has.
_Avoid_: sidebar, stack

**Active panel**:
The pane in the rail that currently receives list input and owns the detail pane's selection.
_Avoid_: focused pane (Focus is list vs detail)

**Collapse**:
The shrinking of an inactive rail pane: shrink-to-fit names when roomy, a 1-row title+count when tight.

**Tier**:
A layout size band. The ladder has two placements: stacked (body width under 80) and beside (otherwise). Below that sits the **floor**: a frame of 22 rows or fewer, or 60 columns or fewer, where chrome is shed. Nothing refuses to draw at any size.

**Detail pane**:
The view of the current selection. Beside the rail on medium and wide terminals; below it when stacked. For containers it has two detail tabs (Logs, Inspect) and the strip; images and volumes have Inspect only.

**Strip**:
The persistent telemetry view at the top of the detail pane for the selected container: cpu and memory sparks, plus network and disk rates.
_Avoid_: dashboard, graph (the sparkline is the glyph, not the view)

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
The persistent bar of context-sensitive key hints plus the status cluster (service state, CLI version, poll spinner).
_Avoid_: status bar (ambiguous with the status cluster)

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

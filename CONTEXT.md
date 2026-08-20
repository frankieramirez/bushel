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
One of the three entity views (containers, images, volumes). One pane is shown at a time.
_Avoid_: tab, screen, page

**Detail pane**:
The right-hand half of the layout, always showing a view of the current selection. For containers it has two detail tabs (Logs, Inspect); images and volumes have Inspect only.

**Focus**:
Which side of the layout receives input — the entity list or the detail pane. Enter moves focus into the detail pane; Esc returns it to the list.

**Zoom**:
Toggling the focused pane to fullscreen and back.

**Filter**:
Fuzzy narrowing of the current entity list; Esc clears it.
_Avoid_: search

**Action menu**:
The bottom sheet opened with `space`, listing the valid actions for the current selection alongside their direct keys.
_Avoid_: context menu, palette

**Bottom bar**:
The persistent bar of context-sensitive key hints plus the status cluster (service state, CLI version, poll spinner).
_Avoid_: status bar (ambiguous with the status cluster)

**Message log**:
The scrollback of recent errors and notices, holding the full stderr behind each status-bar one-liner.
_Avoid_: console

**Splash**:
The animated launch screen that plays only while the startup probes run; any key skips it, and it never adds latency of its own.
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

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
One of the three entity views (containers, images, volumes).
_Avoid_: tab, screen, page

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

### Engine

**Runner**:
The mockable seam through which every `container` CLI invocation passes.
_Avoid_: client, executor

**Poll tick**:
One iteration of the periodic refresh loop that re-lists entities and samples stats. Apple Containers has no event API, so polling is the only refresh model.

**Service**:
The Apple Containers system service (`container system …`) that daemon-backed commands require. bushel probes it at startup and offers a one-key start when it is down.
_Avoid_: daemon

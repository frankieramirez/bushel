# PROTOTYPE — bushel v0.1 layout mock

Throwaway Ratatui prototype for [wayfinder ticket #7](https://github.com/frankieramirez/bushel/issues/7).
Fake data only — it never touches the `container` CLI. Do not merge to main; the branch is the artifact.

## Run

```sh
cd prototype-layout
cargo run                    # full experience
cargo run -- --no-splash     # skip the splash
cargo run -- --reduced-motion  # all animation off (the config escape hatch)
```

On quit it prints frame count and worst draw time (the CPU-cost check for the ambient effect).

## What to react to

- **Splash-as-loading**: plays only while fake probes run (~1.4s), any key skips, dissolves into the layout.
- **Layout**: 45/55 list/detail split, `1/2/3` + `Tab` pane switch (sweep transition), `Enter`/`Esc` focus, `f` zoom.
- **Detail tabs**: `l` Logs (follow sticks to bottom, `F` pauses) / `i` Inspect on containers.
- **Action menu**: `space` bottom sheet, destructive actions tinted, direct keys work without it.
- **Confirm**: `d`/`K`/`P` show the exact command; `y` runs, entity goes pending (spinner) and resolves ~2s later.
- **Errors**: delete an in-use volume (pane `3`, `pg-data`, `d`) → status-bar one-liner, full stderr in `m` message log.
- **Pull**: pane `2`, `u` → modal input, progress streams in the detail pane, never holds a modal.
- **Exec**: `e` on a container really suspends the TUI into `/bin/sh`; `exit` fades back in.
- **Ambient effect** (prototype-gated): slow hue drift on the `bushel` wordmark. `F1` toggles it — is it noise?

Sim keys: `F1` ambient · `F2` service-down takeover (with fake `s` start stream) · `F3` external stop · `b` dismiss version banner · `?` help.

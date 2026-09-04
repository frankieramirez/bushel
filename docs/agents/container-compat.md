# Container CLI compatibility

How bushel treats Apple `container` CLI versions, and the required pass before bumping `TESTED_MIN` / `TESTED_MAX` in `src/client/version.rs`.

## Untested versions

Outside `TESTED_MIN` / `TESTED_MAX`, warn and still run. The banner is dismissible with `b`. Never hard-fail the TUI on an untested CLI.

## Compat pass (required before a range bump)

All of these, in order:

1. Target that `container` CLI on macOS Apple silicon.
2. `cargo test` green (fixture and Client coverage included).
3. Smoke the four panes: list refresh, logs follow, inspect, and one non-destructive lifecycle action (for example stop/start on a disposable container).
4. Diff that CLI's list/inspect JSON and relevant flags against the previous tested minor. Any break is a Client change (command build, parse, or error classify) in the same change that bumps the range.
5. Only then edit `TESTED_MIN` / `TESTED_MAX`. Prefer extending `TESTED_MAX`. Raise `TESTED_MIN` only when intentionally dropping an old minor.

## Where version branches live

Only in Client. Engine and UI stay version-agnostic. Prefer one tolerant parse path. Branch on version only when flags or JSON truly diverge.

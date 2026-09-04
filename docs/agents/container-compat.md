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

## macOS 27 release gate

Do not claim macOS 27 support until the release gate passes on physical Apple silicon or a self-hosted Apple silicon runner. Hosted CI runners do not exercise Apple Containers virtualization.

Run the full matrix below with each combination:

| macOS | `container` CLI |
| --- | --- |
| macOS 26.6 | 1.2.2 and 1.3.1 |
| macOS 27 RC | 1.2.2 and 1.3.1 |
| macOS 27 final | 1.2.2 and 1.3.1 |

For every combination, verify startup, all four panes, stats refresh, inspect, log following, interactive exec, stop/start, image pull, and disposable volume operations. Check published TCP ports through `127.0.0.1`, run one `linux/amd64` container without a separately installed Rosetta package, and preserve existing container state across the OS upgrade.

Test the supported installation and update channels: Homebrew install and update, the curl installer, and `bushel update`. Verify launch, terminal restoration, resizing, Ctrl-C, config persistence, and Gatekeeper behavior.

Record any Apple networking, TCC, or virtualization regression and report it upstream. Do not ship a system-wide privacy workaround. If the RC or final gate fails, document the failure and withhold the macOS 27 support claim until a rerun passes.

## Where version branches live

Only in Client. Engine and UI stay version-agnostic. Prefer one tolerant parse path. Branch on version only when flags or JSON truly diverge.

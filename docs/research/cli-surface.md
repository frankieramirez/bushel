# Research: the `container` CLI as a programmatic surface

**Ticket:** [#3](https://github.com/frankieramirez/bushel/issues/3) · **Map:** [#1](https://github.com/frankieramirez/bushel/issues/1)
**Environment:** `container CLI version 1.2.0 (build: release, commit: 6e65319)` on macOS 26 (Apple silicon). All output below was captured by actually running the CLI on 2026-08-19.

## Summary

The CLI is a workable programmatic surface: every *list*-style command supports `--format json`, `inspect` commands always emit JSON, exit codes propagate faithfully, and errors go to stderr with data on stdout. The gaps: **there is no event/watch mechanism at all** (the TUI must poll), `logs --follow` **does not terminate when the container stops**, stats streaming is TTY-oriented (JSON mode is effectively single-shot), stopped containers expose **no exit code**, and error messages are unstructured prose with no stable shape. Upstream guarantees output stability only within patch versions, and the 1.0.0 release changed every JSON output shape — so the wrapper should treat parsed shapes as version-fragile and check `container --version` at startup.

## Architecture in one paragraph

`container` is a thin XPC client. The real state lives in `container-apiserver`, a per-user launchd agent, which delegates to XPC helpers (`container-core-images` for images/content store, `container-network-vmnet` for networking) and launches one `container-runtime-linux` helper per container (each container is a lightweight VM). Several top-level subcommands (`image`, `volume`, `network`, `system`, `builder`, `registry`, plus anything unrecognized) are resolved as **plugins** under `/usr/local/libexec/container/plugins/`. The XPC protocol is private and Swift-only, so a Rust TUI realistically wraps the CLI as a subprocess — there is no socket or REST API to talk to directly.

## Machine-readable output: what supports what

`--format` accepts `json`, `table` (default), `yaml`, `toml` wherever it exists.

| Command | `--format json` | Notes |
|---|---|---|
| `container list` / `ls` | ✅ | `-a` includes stopped; `-q` for IDs only; compact (unpretty) JSON array |
| `container inspect <id>…` | (always JSON) | No `--format` flag; always emits a **pretty-printed** JSON array |
| `container stats` | ✅ | JSON mode prints **one** array and exits (see below) |
| `container logs` | ❌ | Raw stream; `-f` follow, `-n` tail, `--boot` for VM boot log |
| `container image list` | ✅ | `-v` adds per-platform rows (table only) |
| `container image inspect` | (always JSON) | Same convention as container inspect |
| `container volume list` | ✅ | |
| `container volume inspect` | (always JSON) | |
| `container network list` | ✅ | |
| `container network inspect` | (always JSON) | |
| `container system status` | ✅ | |
| `container system df` | table only | No `--format` flag in 1.2.0 |
| `container system logs` | ❌ | macOS unified-log lines; `-f` follow, `--last 5m` |
| `container system property list` | TOML text | The config file format itself |
| mutations (`run -d`, `create`, `start`, `stop`, `kill`, `delete`, `volume create/delete`, `image delete`) | n/a | Print the ID/name of each affected object on stdout, one per line |

Two JSON quirks to build the decoder around:

- `ls`-style JSON is compact; `inspect` JSON is pretty-printed. Both are arrays even for a single object.
- Slashes are escaped (`"docker.io\/library\/alpine:latest"`) — standard JSON, harmless to any real parser, but visible in fixtures.

## Captured output shapes

### `container ls --format json` (one running container)

Each element has three top-level keys: `configuration` (immutable creation-time spec), `id`, `status` (runtime state). Trimmed but structurally complete:

```json
[{
  "configuration": {
    "creationDate": "2026-08-20T01:46:35Z",
    "id": "qtest",
    "image": {
      "descriptor": {"digest": "sha256:28bd5fe…", "mediaType": "application/vnd.oci.image.index.v1+json", "size": 9218},
      "reference": "docker.io/library/alpine:latest"
    },
    "initProcess": {"arguments": ["-c", "…"], "executable": "sh", "terminal": false, "workingDirectory": "/", "user": {"id": {"gid": 0, "uid": 0}}},
    "labels": {}, "mounts": [],
    "networks": [{"network": "default", "options": {"hostname": "qtest", "mtu": 1280}}],
    "platform": {"architecture": "arm64", "os": "linux"},
    "publishedPorts": [], "publishedSockets": [],
    "resources": {"cpuOverhead": 1, "cpus": 4, "memoryInBytes": 1073741824},
    "runtimeHandler": "container-runtime-linux"
  },
  "id": "qtest",
  "status": {
    "networks": [{"hostname": "qtest", "ipv4Address": "192.168.64.2/24", "ipv4Gateway": "192.168.64.1", "macAddress": "f6:8e:b2:e0:41:cc", "network": "default"}],
    "startedDate": "2026-08-20T01:46:37Z",
    "state": "running"
  }
}]
```

After `container stop`, the same object's status collapses to:

```json
{"networks": [], "startedDate": "2026-08-20T01:48:05Z", "state": "stopped"}
```

**There is no exit code anywhere** — not in `ls`, not in `inspect`, not in the table view. The only way to observe a container's exit code is to be the process that ran it in the foreground (`container run` / `start --attach` propagate it as the CLI's own exit code; verified `run --rm alpine sh -c 'exit 7'` → CLI exits 7, and `exec … sh -c 'exit 3'` → exits 3). Default `ls` shows only running containers; a TUI always wants `ls -a`.

### `container image ls --format json`

Array of `{configuration: {name, creationDate, descriptor{digest, mediaType, size}}, id, variants: [...]}` where `variants` contains a full OCI config (architecture, os, history, rootfs diff_ids, size) **per platform** — an alpine pull produced 16 variants (8 platforms + attestation manifests with `"os": "unknown"`). Filter variants to the host platform (or ones with a non-`unknown` os) before display.

### `container volume ls --format json`

```json
[{"configuration": {"creationDate": "2026-08-20T01:47:42Z", "driver": "local", "format": "ext4",
  "labels": {}, "name": "qvol", "options": {}, "sizeInBytes": 549755813888,
  "source": "/Users/f/Library/Application Support/com.apple.container/volumes/qvol/volume.img"}, "id": "qvol"}]
```

(`sizeInBytes` is the sparse maximum — 512 GiB by default — not actual usage.)

### `container system status --format json`

```json
{"apiServerAppName": "container-apiserver", "apiServerBuild": "release",
 "apiServerCommit": "6e65319fe476ffe8db8ddaf828a537ed36fe2859",
 "apiServerVersion": "container-apiserver version 1.2.0 (build: release, commit: 6e65319)",
 "appRoot": "/Users/f/Library/Application Support/com.apple.container/",
 "installRoot": "/usr/local/", "status": "running"}
```

This is the only JSON-emitting command that returns an object rather than an array.

## Logs

- `container logs qtest` — full stdio log; `-n 3` tails; `--boot` returns kernel/boot output of the VM. Log lines are raw (no timestamps, no stream tagging — stdout and stderr are interleaved without markers).
- `container logs -f qtest` streams new lines with ~1s latency. **Verified: when the container stops, `-f` does not exit** — it keeps blocking indefinitely. The wrapper owns follow-process lifecycle: kill the subprocess when the pane closes *and* when a poll shows the container left the running state.
- `container logs nosuch` → exit 1, stderr:
  `Error: failed to get logs for container nosuch (cause: "internalError: "failed to open container logs: notFound: "container with ID nosuch not found""")`

## Stats

- `container stats --no-stream --format json` → single compact array, exit 0:

  ```json
  [{"blockReadBytes": 3981312, "blockWriteBytes": 0, "cpuUsageUsec": 35372, "id": "qtest",
    "memoryLimitBytes": 1073741824, "memoryUsageBytes": 4780032,
    "networkRxBytes": 29461, "networkTxBytes": 602, "numProcesses": 2}]
  ```

- **`--format json` without `--no-stream` also prints exactly one array and exits** (~2s runtime, verified twice). JSON stats is effectively always single-shot; there is no NDJSON stream.
- Streaming *table* mode is for humans only: it enters the alternate screen (`ESC[?1049h`), redraws with cursor-home/clear, and when killed mid-stream can emit `error collecting stats: CancellationError()`. Never wrap it.
- `cpuUsageUsec` is a cumulative counter — CPU% requires two samples: `Δusec / Δwall / cores`. The table view computes this internally (shows `Cpu %`), so a JSON-based TUI must sample twice itself.

## Exit codes and error shapes

Errors go to **stderr**; stdout stays clean (verified: `container ls --format json 2>/dev/null` with the service down prints nothing). Errors are plain text, never JSON. Three exit-code bands observed:

| Situation | Exit | stderr (captured verbatim) |
|---|---|---|
| Success | 0 | — |
| Service not running (any daemon-backed command) | 1 | `Error: internalError: "failed to list containers" (cause: "interrupted: "XPC connection error: Connection invalid"")`<br>`Ensure container system service has been started with `container system start`.` |
| `system status`, service down | 1 | `apiserver is not running and not registered with launchd` (on stdout, exit 1) |
| `inspect` unknown container | 1 | `Error: container not found: nosuch` |
| `start` unknown container | 1 | `Error: get failed: container nosuch not found` |
| `stop` unknown container | 1 | `Error: internalError: "failed to stop container" (cause: "notFound: "container with ID nosuch not found"")` |
| `kill` unknown container | 1 | `Error: internalError: "failed to kill container" (cause: "notFound: "container with ID nosuch not found"")` |
| `delete` a running container (no `-f`) | 1 | `Error: internalError: "failed to delete container" (cause: "invalidState: "container qtest is running and can not be deleted"")` |
| `image inspect` unknown image | 1 | `Error: image not found: nosuch:latest` |
| `volume inspect` unknown volume | 1 | `Error: volume not found: nope` |
| Unknown flag | 64 | `Error: Unknown option '--bogus'` + usage |
| Unknown subcommand (plugin lookup fails) | 64 | `Error: Plugins are unavailable. Start the container system services and retry: …` + usage |
| Foreground `run`/`exec` | container's exit code | verified 7 and 3 propagate |

Note the inconsistency: the same logical error ("container not found") has at least three different phrasings depending on the command (`container not found: X`, `get failed: container X not found`, nested `notFound: "container with ID X not found"`). Error classification should be by regex over a small set of keywords (`not found`, `XPC connection error` / `Ensure container system service`, `invalidState`, `is running and can not be deleted`) rather than exact-match, and must tolerate rewording across versions.

## Service lifecycle gotchas

- With the service **stopped**, `--help` still works everywhere (help is client-side), but plugin subcommands that don't exist error with exit 64, and all daemon-backed commands fail with the XPC exit-1 error above. `container system status` is the cheap, reliable health probe (exit 0 running / exit 1 not).
- **`container system start` can prompt interactively.** On a fresh install with no default kernel it asks `Install the recommended default kernel …? [Y/n]:`; with no TTY it dies with `Error: failed to read user input` (exit 1) — *while still having launched the apiserver*, leaving a half-initialized state (status says running, but containers can't start). The non-interactive form is `container system start --enable-kernel-install` (there is also `--disable-kernel-install`); kernel install takes tens of seconds on first run. A TUI offering "start the service" must use the explicit flag, never bare `system start`.
- `container system stop` stops **all running containers** as a side effect, then unloads the launchd services. Surface that as a destructive action.
- First `container run` after install pulls a ~66 MB init image and the kernel in addition to the requested image; progress goes to **stderr** (verified stdout carried only the container's output), respecting `--progress plain|none` for non-TTY use.

## Events / watch: none — polling is mandatory

- No `events`, `watch`, or `subscribe` subcommand exists in 1.2.0 (checked full help tree).
- Neither the README, `docs/technical-overview.md`, nor the 0.10.0–1.2.2 release notes mention any event or notification mechanism. State changes are only observable by re-running `ls -a` / `stats`.
- The XPC APIs between CLI and apiserver are internal; even if they carry notifications, they're Swift/XPC-only and explicitly version-gated (1.0.0 "removed compatibility with application major version 0 XPC APIs").
- `container system logs -f` tails apiserver unified-log lines that *do* contain lifecycle breadcrumbs (e.g. `container finished in exit monitor … rc=ExitStatus(exitCode: 0, …)` — the only place an exit code appears at all), but the format is human log text and clearly not a contract. Tempting, not load-bearing.

## Stability policy (upstream)

From the [apple/container README](https://github.com/apple/container) and release notes:

- "Its stability, both for consuming the project as a Swift package and the `container` tool, is only guaranteed within **patch versions**, such as between 0.1.1 and 0.1.2." The README does not state a stronger post-1.0 guarantee.
- Evidence this is real: the **1.0.0** release notes list changed "JSON/YAML/TOML output shapes … for container, image, network, volume operations" and removed `system property get/set` subcommands. `--format` values themselves have churned (YAML added in 0.12.0).
- Release cadence is roughly monthly (1.0.0 → 1.1.0 → 1.2.0 → 1.2.2 within ~9 weeks); macOS 26 is the only supported OS.
- API docs at apple.github.io/container document the Swift package, not a CLI contract.

## Implications for the wrapper architecture

(bushel is a Rust + Ratatui TUI spawning the CLI via tokio subprocesses.)

1. **One shape for reads: `spawn → stdout JSON → serde`.** Every read the TUI needs (`ls -a`, `image ls`, `volume ls`, `network ls`, `system status`, `stats --no-stream`) supports `--format json`; `inspect` is JSON by default. Put a Runner-style trait seam over the subprocess (`async fn run_json(&self, args: &[&str]) -> Result<Vec<u8>, CliError>` on a `Runner` trait, real impl on `tokio::process::Command`, mock impl replaying captured fixtures) so every view is testable without the daemon. Deserialize with serde into structs mirroring the `configuration`/`id`/`status` top-level fields; **do not** use `#[serde(deny_unknown_fields)]` — shapes drift between versions, and unknown-field tolerance (serde's default) is the compatibility margin.

2. **Poll; don't look for events.** There is no watch mechanism, full stop. A `tokio::time::interval` poller running `container ls -a --format json` (and `stats --no-stream --format json` when a stats pane is visible), feeding snapshots to the Ratatui event loop over an mpsc channel, is the state engine. Measured latency of `ls` round-trips is well under a second; 1–2 s ticks are comfortable. Diff successive snapshots keyed on `id` to synthesize the events the UI wants (created/started/stopped/deleted).

3. **The wrapper owns every long-lived subprocess.** `logs -f` never exits on container stop — spawn it with `kill_on_drop(true)`, read lines into the channel via a task, and abort the task (or signal a `CancellationToken`) when the pane closes *and* when the poller sees the container leave `running` (or keep it and mark the pane "container stopped; stream idle" — but never `.wait()` on it). Same for `system logs -f`.

4. **Stats need client-side computation.** JSON stats is single-shot with a cumulative `cpuUsageUsec`; the TUI must keep the previous sample per container and derive CPU% from deltas (`Δusec / Δwall / cores`). Never invoke streaming table mode (its alt-screen ANSI would fight Ratatui for the terminal).

5. **Exit codes are only visible at run time.** Nothing in `ls`/`inspect` records why a container stopped. If bushel wants "exited (7)" badges it must either (a) be the invoker (`run`/`start -a` propagate codes) or (b) accept `stopped` without a code. Design the model so exit code is `Option<i32>`, usually `None`.

6. **Error handling = exit code + stderr regex, defensively.** 0 success; 64 usage/parse errors (bug in the wrapper's own arg construction); 1 everything else, classified by substring into a `CliError` enum: `XPC connection error` / `Ensure container system service` → `ServiceDown` (offer to start it), `not found` → `NotFound` (stale UI entry; trigger immediate re-poll), `invalidState` / `is running and can not be deleted` → `InvalidState` (offer `-f`), everything else → `Other(String)`. Treat classification as best-effort; always keep the raw stderr in the error value and available in the UI, since phrasings vary per command and per version.

7. **Handle the daemon-down and first-run states as first-class screens.** Probe with `container system status` on startup (exit 1 → "service not running" screen with a start action). Start it with `container system start --enable-kernel-install` — the bare command can block on an interactive prompt and there is no TTY to answer it. Warn before `system stop` (kills all containers).

8. **Pin expectations to the CLI version.** Upstream guarantees output stability only within patch versions and has changed all JSON shapes at 1.0.0. On startup run `container --version`, parse `container CLI version X.Y.Z`, and (at minimum) warn on minor/major versions newer than tested; keep captured JSON fixtures per version and replay them through the mock `Runner` in regression tests. Target 1.2.x now; expect to re-validate shapes on each minor bump. `container` requires macOS 26+, which bounds the support matrix nicely.

9. **Non-TTY hygiene is already decent.** Data on stdout, errors and progress on stderr, `--progress plain|none` for pulls/runs, exit codes honest. Spawn every subprocess without a PTY and set `--progress none` (or parse `plain` progress lines from stderr to drive a Ratatui gauge for `image pull`).

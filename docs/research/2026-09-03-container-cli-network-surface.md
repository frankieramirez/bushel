# What network-related subcommands and output shapes does Apple `container` CLI expose, how stable across 1.x, and what is “settled enough” for a read-only pane?

## Findings

### Lead answer

On current Apple `container` (verified against tagged release **1.3.1** and matching docs), the dedicated network surface is `container network` (alias `n`) with five subcommands: **`create`**, **`delete`/`rm`**, **`list`/`ls`**, **`inspect`**, and **`prune`**. User-defined network management is documented as **macOS 26+**; `system start` always creates a builtin **`default`** vmnet network. For a read-only Scry pane, treat **`container network list --format json`** (and optionally **`inspect`**) as the settled contract: JSON is a `NetworkResource` array with top-level keys **`id`**, **`configuration`**, **`status`**, stabilized as a breaking cleanup in **1.0.0** and left alone through **1.1–1.3** release notes. Do **not** parse the human table as API (docs historically showed a `STATE` column; 1.x code emits only `NETWORK` / `SUBNET`).

### Subcommand inventory (network group)

Registered in `NetworkCommand` (`commandName: "network"`, alias `"n"`):

| Subcommand | Aliases | Mutating? | Role |
|---|---|---|---|
| `create` | — | yes | Create named network; prints network id |
| `delete` | `rm` | yes | Delete named networks or `--all` (non-builtin); prints deleted ids |
| `list` | `ls` | no | List networks (table / json / yaml / toml / quiet) |
| `inspect` | — | no | JSON detail for named networks |
| `prune` | — | yes | Delete unused non-builtin networks; prints pruned ids |

Sources: [NetworkCommand.swift @ 1.3.1](https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkCommand.swift), [command-reference “Network Management (macOS 26+)” @ 1.3.1](https://github.com/apple/container/blob/1.3.1/docs/command-reference.md).

#### `create`

```bash
container network create [--internal] [--label <label> ...] [--option <option> ...] \
  [--plugin <plugin>] [--subnet <subnet>] [--subnet-v6 <subnet-v6>] [--debug] <name>
```

- Default plugin: `container-network-vmnet`.
- `--internal` → `NetworkMode.hostOnly`; otherwise `nat`.
- Stdout: single network id (`print(network.id)`).

Sources: [command-reference](https://github.com/apple/container/blob/1.3.1/docs/command-reference.md), [NetworkCreate.swift @ 1.3.1](https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkCreate.swift), [NetworkMode.swift @ 1.3.1](https://github.com/apple/container/blob/1.3.1/Sources/ContainerResource/Network/NetworkMode.swift).

#### `delete` / `rm`

```bash
container network delete [--all] [--debug] [<network-names> ...]
```

- Refuses builtin networks; `--all` filters `!$0.isBuiltin`.
- Stdout: one deleted id per successful delete.
- Fails if IPs still in use (allocator disable).

Source: [NetworkDelete.swift @ 1.3.1](https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkDelete.swift).

#### `list` / `ls`

```bash
container network list [--format <format>] [--quiet] [--debug]
```

- Formats (docs): `json`, `table`, `yaml`, `toml` (default `table`).
- `-q` / `--quiet`: only network name/id.
- Despite “user-defined” wording in the command reference, list returns the full set including **`default`** (shown in networking how-to).

#### `inspect`

```bash
container network inspect <networks> ... [--debug]
```

- Emits JSON for the requested names; errors with not-found if any missing.
- Implementation encodes the same `NetworkResource` values as list (pretty JSON).

Sources: [NetworkInspect.swift](https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkInspect.swift), [NetworkList.swift](https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkList.swift).

#### `prune`

```bash
container network prune [--debug]
```

- Deletes non-builtin networks with no container attachments; prints pruned ids.
- Preserves default/system networks.

Source: [NetworkPrune.swift @ 1.3.1](https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkPrune.swift).

### Adjacent network-related CLI (not under `network`)

- **Attach**: `container run` / `create` `--network <name>[,mac=XX:XX:XX:XX:XX:XX][,mtu=VALUE]`.
- **Ports**: `-p` / `--publish` `[host-ip:]host-port:container-port[/protocol]`.
- **DNS (host resolver)**: `container system dns create|delete|list` (sudo for create/delete).
- **Config defaults**: `~/.config/container/config.toml` `[network]` (`subnet`, `subnetv6`) and `[dns]` (`domain`).
- **Per-container endpoints**: `container inspect` / `container ls --format json` → `status.networks[]`.

Sources: [command-reference](https://github.com/apple/container/blob/1.3.1/docs/command-reference.md), [networking.md @ 1.3.1](https://github.com/apple/container/blob/1.3.1/docs/networking.md), [container-inspection.md @ 1.3.1](https://github.com/apple/container/blob/1.3.1/docs/container-inspection.md).

### Output shapes

#### Table (`list`, default)

From `NetworkResource+ListDisplayable` @ 1.3.1:

- Header: **`NETWORK`**, **`SUBNET`**
- Row: `[id, status.ipv4Subnet.description]`
- Quiet: `id`

Official how-to example (1.3.1) matches:

```text
NETWORK  SUBNET
default  192.168.64.0/24
foo      192.168.65.0/24
```

**Stability note:** Pre–`NetworkResource` / early docs (and PR #243 demos) used **`NETWORK STATE SUBNET`**. That `STATE` column is **gone** in 1.x list display code. Prefer JSON for tooling.

Sources: [NetworkResource+ListDisplayable.swift @ 1.3.1](https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkResource+ListDisplayable.swift), [networking.md @ 1.3.1](https://github.com/apple/container/blob/1.3.1/docs/networking.md), [PR #243](https://github.com/apple/container/pull/243).

#### JSON (`list --format json` and `inspect`)

`NetworkResource` encodes **three top-level keys**:

```json
{
  "id": "<name>",
  "configuration": { /* NetworkConfiguration */ },
  "status": { /* NetworkStatus */ }
}
```

**`configuration` fields** (encode path @ 1.3.1): `name`, `creationDate`, `mode` (`"nat"` | `"hostOnly"`), optional `ipv4Subnet` / `ipv6Subnet`, `labels`, `plugin`, `options`. Decoder still accepts legacy `id`, `subnet`, and `pluginInfo` for stored configs.

**`status` fields**: `ipv4Subnet`, `ipv4Gateway`, optional `ipv6Subnet`.

`id` / `name` are the same string (network name is the identity). Builtin networks are labeled (`isBuiltin` derived from labels) and cannot be deleted.

Sources: [NetworkResource.swift](https://github.com/apple/container/blob/1.3.1/Sources/ContainerResource/Network/NetworkResource.swift), [NetworkConfiguration.swift](https://github.com/apple/container/blob/1.3.1/Sources/ContainerResource/Network/NetworkConfiguration.swift), [NetworkStatus.swift](https://github.com/apple/container/blob/1.3.1/Sources/ContainerResource/Network/NetworkStatus.swift).

#### Container attachment JSON (for correlating “who’s on this network”)

From inspection docs @ 1.3.1, running containers expose:

```json
"status": {
  "state": "running",
  "networks": [
    {
      "ipv4Address": "192.168.64.3/24",
      "ipv4Gateway": "192.168.64.1",
      "hostname": "my-web-server.test.",
      "network": "default"
    }
  ]
}
```

Source: [container-inspection.md @ 1.3.1](https://github.com/apple/container/blob/1.3.1/docs/container-inspection.md).

### Stability across 1.x

| Release | Network CLI / shape notes |
|---|---|
| **≤0.x / PR #243** | Introduced `container network` create/delete/list/inspect for macOS 26; table showed `STATE`; early printable models differed. |
| **1.0.0** (2026-06-09) | **Breaking:** cleaned structured JSON for network `ls`/`inspect` to ManagedResource shape (`id` + `configuration` + `status`); `NetworkConfiguration` prefers `name` over `id`; removed 0.x XPC API compatibility; IP-lease fix for address leaks. Tracked via [#1623](https://github.com/apple/container/issues/1623) / PR #1624. |
| **1.1.0** | Reliability: always update default network from system config; remove network variant computation from API server; no new network subcommands / no highlighted JSON shape break. |
| **1.2.0–1.2.2** | No network-group CLI breaks called out in release highlights (security/core/k8s focus). |
| **1.3.0–1.3.1** | No network-group CLI breaks; docs reorg; security patches. Current surface matches 1.3.1 sources above. |

**Practical reading:** Subcommand set (`create`, `delete`/`rm`, `list`/`ls`, `inspect`, `prune`) and the **`id`/`configuration`/`status` JSON envelope** have been stable from **1.0.0 through 1.3.1**. The hard break vs tooling is **0.x → 1.0**. Within 1.x, create flags gained plugin/`--option`/`--internal` polish, but read paths stayed on the same resource type.

Sources: [1.0.0 release](https://github.com/apple/container/releases/tag/1.0.0), [1.1.0](https://github.com/apple/container/releases/tag/1.1.0), [1.2.0](https://github.com/apple/container/releases/tag/1.2.0), [1.3.0](https://github.com/apple/container/releases/tag/1.3.0), [1.3.1](https://github.com/apple/container/releases/tag/1.3.1), [#1623](https://github.com/apple/container/issues/1623).

### “Settled enough for a read-only pane” bar

Recommend treating the surface as **settled for a read-only Networks pane** if all of the following hold:

1. **CLI major ≥ 1** (require `container system version` ≥ **1.0.0**; prefer **≥ 1.1.0** for IP-lease / default-network update fixes).
2. **Host gate:** user-defined networks need **macOS 26+**; still show builtin `default` when present after `system start` on supported hosts.
3. **Read path only:** use `container network list --format json` as the primary poll; use `container network inspect <id>` for detail; optionally join with `container ls --format json --all` / `container inspect` via `status.networks[].network`.
4. **Parse JSON, not table** — ignore `STATE` if any older string appears; columns are not the contract.
5. **Model the documented fields:** at minimum `id`, `configuration.mode`, `configuration.plugin`, `configuration.labels` (for builtin), `status.ipv4Subnet`, `status.ipv4Gateway`, optional IPv6; tolerate extra keys.
6. **Out of pane scope (mutating):** `create`, `delete`/`rm`, `prune`, and host DNS create/delete.
7. **Known product gaps** (display, don’t invent): bare-hostname DNS on custom networks is incomplete ([#1809](https://github.com/apple/container/issues/1809) referenced from networking.md).

That bar is “settled enough”: 1.x did not churn the network list/inspect JSON envelope after the intentional 1.0 cleanup, and Apple’s own docs tell operators to script via `--format json` / `inspect`.

## Sources

- https://github.com/apple/container/blob/1.3.1/docs/command-reference.md — Official CLI reference for `network create|delete|prune|list|inspect` and `--network` / DNS / publish flags; macOS 26+ note.
- https://github.com/apple/container/blob/1.3.1/docs/networking.md — Operator guide: default network, custom networks, table example (`NETWORK`/`SUBNET`), config.toml `[network]`, DNS caveats.
- https://github.com/apple/container/blob/1.3.1/docs/container-inspection.md — Container JSON shape for `status.networks[]` (`ipv4Address`, `ipv4Gateway`, `hostname`, `network`).
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkCommand.swift — Subcommand registration + `n` alias.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkCreate.swift — Create flags, modes, stdout id.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkDelete.swift — Delete/rm semantics, builtin refusal, stdout ids.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkList.swift — List formats via shared `Output.render`.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkInspect.swift — Inspect JSON emission + missing-name errors.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkPrune.swift — Prune unused non-builtin networks.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerCommands/Network/NetworkResource+ListDisplayable.swift — Table columns `NETWORK`/`SUBNET` (no `STATE`).
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerResource/Network/NetworkResource.swift — JSON envelope `id`/`configuration`/`status`.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerResource/Network/NetworkConfiguration.swift — Config fields + legacy decode keys.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerResource/Network/NetworkStatus.swift — Runtime address fields.
- https://github.com/apple/container/blob/1.3.1/Sources/ContainerResource/Network/NetworkMode.swift — `nat` / `hostOnly`.
- https://github.com/apple/container/releases/tag/1.0.0 — Breaking JSON cleanup for network ls/inspect; IP lease fix.
- https://github.com/apple/container/releases/tag/1.1.0 — Network reliability changes; no shape break called out.
- https://github.com/apple/container/releases/tag/1.2.0 — No network CLI break highlights.
- https://github.com/apple/container/releases/tag/1.3.0 — Docs reorg; no network CLI break highlights.
- https://github.com/apple/container/releases/tag/1.3.1 — Current patch baseline used for source/doc pins.
- https://github.com/apple/container/issues/1623 — Intent to normalize network/volume JSON (closed via PR #1624 into 1.0).
- https://github.com/apple/container/pull/243 — Original `container network` introduction / early help text and `STATE` table demo.

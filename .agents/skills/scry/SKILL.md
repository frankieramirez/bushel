---
name: scry
description: "Plan a chunk of work too big for one agent session as a shared map of decision tickets on GitHub, and resolve them one at a time. Use when asked to scry, wayfinder, chart a map, walk a map, take the next ticket on the map, or /scry."
argument-hint: "[loose idea | map number | ticket number | issue URL] [you-pick]"
disable-model-invocation: true
---

# Scry

A loose idea has arrived, too big for one session. The way to the destination is still fog. This skill charts that way as a shared map on GitHub, then works **decision tickets** (questions whose answer is a decision) one at a time until the route is clear.

The destination is named first. It might be a spec, a locked decision, or a change made in place. Name it, because every ticket hangs off it.

## Operating principles

- **Decisions, then delivery.** Each ticket resolves a question. The map is done when nothing is left to decide before someone goes and builds. The pull to start building is the signal the map is finished. An effort can override this in its Notes.
- **One ticket per session**, except research tickets created while charting, which resolve in parallel in that same session.
- **Refer by name.** Every map and ticket is an issue with a title. In anything the human reads, use that title. Wrap the link inside the name. A wall of `#42, #43` is illegible.
- **Claim before work.** Assign the ticket to the person driving this session first, so a parallel session skips it. An open unassigned ticket is unclaimed.
- **The map is an index.** A decision lives on its ticket. The map gists it and links. It does not restate the answer.

`SKILL_DIR` is the absolute directory this SKILL.md lives in. The Bash tool forgets variables between calls, so every block that runs the bundled script sets `SKILL_DIR` again on its first line.

## Arguments

Parse tokens, then treat the remainder as the idea, number, or URL.

| Token | Effect |
|-------|--------|
| `you-pick` | On grilling rounds, accept every recommended answer. Same meaning as the user saying "make the decisions" or "you pick". |

**No number or URL (a loose idea).** Chart a new map.

**A number or issue URL.** Load that issue.

- Label `scry:map`: walk that map.
- Label `scry:research`, `scry:prototype`, `scry:grilling`, or `scry:task`: walk its parent map and claim this ticket.
- The same labels under the older `wayfinder:` prefix mean the same thing. Walk them as they are; do not relabel.
- Any other issue: chart a new map whose destination is informed by that issue.

## Execution spine

1. Resolve the tracker (Stage 1).
2. Decide chart vs walk from the arguments (above).
3. Chart: Stage 2, then stop. Walking tickets is a later session.
4. Walk: Stage 3. Resolve one ticket, file new fog, stop.

---

## Stage 1: Tracker

If `docs/agents/issue-tracker.md` exists, read it and follow its "Wayfinding operations" section for any mechanic it specifies (extra labels, owning docs, parent-link fallbacks). Missing file: GitHub via `gh`, using the operations in `references/github-ops.md`.

Load `references/github-ops.md` now. `scripts/map.sh` is the only way to create issues, attach children, wire blocks, query the frontier, and claim. Do not improvise those `gh` calls.

Confirm the host with `gh repo view`. If that fails, stop.

Pass `GH_HOST=<host>` inline on every `map.sh` invocation. Derive the host from `gh repo view --json url --jq .url`, or from the issue URL if one was passed. Shell state does not persist between Bash calls. On `github.com` the prefix can be dropped.

Ensure labels exist once per session:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" ensure-labels
```

Exit 3 from the script means this token cannot write issues (usually HTTP 403). Read `references/scratch.md` and follow it. Do not keep retrying `gh issue create`.

---

## Stage 2: Chart

Load `references/map-shape.md`, `references/grilling.md`, and `references/domain.md`.

### 2a. Name the destination

Grill until the destination is a sentence or two: the spec, decision, or in-place change this map is finding its way to. Update `CONTEXT.md` and ADRs as terms land.

The destination fixes the scope. Work past it belongs in Out of scope on the map, never in the ticket list.

### 2b. Map the frontier

Grill again, breadth-first this time: fan across the space rather than deep on one thread. Surface the open decisions and the first steps takeable now.

If this surfaces no fog (the way is already clear, and the whole journey fits one session), you do not need a map. Stop and ask how they want to proceed.

### 2c. Write the map

Create the map issue, label `scry:map`. Destination and Notes filled in. Decisions so far empty. Fog sketched into **Not yet specified**.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" create-map "Map: <destination in a few words>" <<'EOF'
<body from references/map-shape.md>
EOF
```

Notes record: domain; files every session should read; standing preferences; any owning doc that later resolutions should update (a decision log, a spec).

### 2d. File the tickets you can specify

A ticket is ready to file when you can state its **Question** precisely. Sharpness of the question matters. Whether you can answer it yet does not.

Create each one as a child of the map, labelled `scry:<type>` (`research`, `prototype`, `grilling`, `task`). See Ticket types in `references/map-shape.md`.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" create-ticket MAP_NUMBER TYPE "Title" <<'EOF'
## Question

<the decision or investigation>
EOF
```

Wire blocking edges in a **second pass**, once every ticket has a number:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" wire CHILD_NUMBER BLOCKER_NUMBER
```

Everything still too dim to phrase stays in **Not yet specified**. Do not pre-slice fog into ticket-sized pieces.

### 2e. Fire research

For each `research` ticket just created, read `references/research.md` and spawn a generic subagent seeded with that file plus the ticket's Question. They run as one concurrent batch. Charting hand-resolves nothing else.

Stop. Charting is one session.

---

## Stage 3: Walk

Load `references/map-shape.md` if you have not this session.

### 3a. Load the map

Fetch the map issue (the low-resolution view). Do not fetch every child body yet.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" view MAP_NUMBER
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" frontier MAP_NUMBER
```

Orient to Destination and Notes before picking a ticket.

### 3b. Choose and claim

If the user named a ticket, use it. Otherwise take the first frontier row (open, unblocked, unclaimed, map order).

Claim it before any work:

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" claim TICKET_NUMBER
```

### 3c. Resolve

Fetch that ticket's body. Zoom into a related or closed ticket only when this question needs it.

Load the reference for its type, and only that type:

| Type | Load |
|------|------|
| `grilling` | `references/grilling.md` and `references/domain.md` |
| `research` | `references/research.md` |
| `prototype` | `references/prototype.md`, then `references/grilling.md` once there is an artifact to react to |
| `task` | none. Do the work, or hand the user a precise checklist |

If Notes name more files to read, read them. When the type is unclear, load grilling and domain.

`you-pick` (or the user saying "make the decisions") accepts recommended grilling answers.

### 3d. Record

Post the answer as a comment, close the ticket, append one gist line to the map's **Decisions so far**.

```bash
SKILL_DIR="<absolute path of the directory containing this SKILL.md>";
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" comment TICKET_NUMBER <<'EOF'
<answer>

Docs impact: <owning doc and what changes, or none>
EOF
GH_HOST=<derived-host> bash "$SKILL_DIR/scripts/map.sh" close TICKET_NUMBER
```

Then `view` the map, splice a line under **Decisions so far**, and `update-body` the map:

```
- [<ticket title>](<url>): <one-line gist>
```

If Notes or the repo name an owning document, write the decision there too. The ticket still holds the full answer.

Assets created while resolving (research notes, a prototype path) are linked from the comment. Do not paste them into the map.

### 3e. Graduate fog

Create-then-wire any question the answer just made specifiable. Clear each graduated patch from **Not yet specified** so it lives only as its new ticket.

If this answer shows a ticket sits past the destination, close that ticket and move one line into **Out of scope** (gist, why, link). It does not go in Decisions so far.

If the decision invalidates other tickets, update or close them.

Stop. One ticket is the session.

---

## References

| Reference | Load at | Purpose |
|-----------|---------|---------|
| `references/github-ops.md` | Stage 1 | How `map.sh` talks to GitHub, including exit 3 |
| `references/map-shape.md` | Stage 2, Stage 3 | Map body, ticket types, fog, out of scope |
| `references/grilling.md` | Stage 2; Stage 3 on grilling or prototype | Design-tree interview |
| `references/domain.md` | With grilling | Glossary and ADRs as terms land |
| `references/research.md` | Stage 2e; Stage 3 on research | AFK cited notes under `docs/research/` |
| `references/prototype.md` | Stage 3 on prototype | Cheap artifact to react to |
| `references/scratch.md` | Stage 1, exit 3 only | Local map when GitHub writes fail |

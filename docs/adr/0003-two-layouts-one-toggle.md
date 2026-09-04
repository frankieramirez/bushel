# 0003 — Two body layouts, one toggle

## Status

Accepted (2026-09-04, [Bushel TUI Directions](https://claude.ai/design/p/f1c73b54-e665-41fc-b7e6-c65afb069802) turn 1, directions 1b and 1c)

## Context

[ADR 0002](0002-unified-rail.md) put all four panes in a rail at every size. Drawing the shipped build at real character cells at 120×40 showed what that costs in practice: four bordered boxes spend 8 rows and 8 columns on chrome, the active pane's `Fill(1)` leaves 15 empty rows under four containers while images clip at 7 of 9, and the header's `1 containers` (a keybinding) sits next to the rail's `containers 4` (a count) — same shape, adjacent, different meaning.

Two ways out were drawn. **1b** keeps ADR 0002 and takes the chrome out of it. **1c** rejects its premise: the type is a mode, the split should be horizontal, and both halves should get every column the terminal has.

They are not rankable. The rail wins when the question is "what else is running"; the table wins when the question is "what exactly is this one doing", because logs get 120 or 200 columns instead of 84, and a container row can carry state, uptime, memory ceiling, image, network, and volumes without truncating any of them. Which question a person is asking is not something the program can know.

## Decision

Ship both. `layout = "rail" | "table"` in the config file, `--layout` on the command line, and a settings panel on `,` that toggles it live and writes the file.

**Rail is the default**, because it is the continuation of ADR 0002 and the one that answers the ambient question. It loses its boxes: four typographic sections in one borderless column, separated from the detail pane by a single rule. Focus is carried by the accented section label, the `▎` selection bar, and the rule's colour. Sections shrink to fit and the slack pools once at the bottom, over a footer naming the reclaimable bytes and the key that frees them.

**Table** is the alternative. The header becomes the switcher and carries the counts, which kills the collision by merging the two things rather than separating them.

Four changes ride along and apply to both:

- Header keys are bracketed (`[1] containers`), and the status cluster moves from the bottom bar up into the header.
- The bottom bar's right side lists the actions valid on the selection.
- Registry prefixes collapse to a dim two-letter token (`dh`, `gh`) so image names start at character 4.
- The telemetry strip carries sparklines at every size, in one, two, or three rows depending on the room.

The settings panel is a view of `Config`, not a superset of it: every row is a field in the file, and nothing is toggleable that does not survive a restart.

## Consequences

- ADR 0002 still describes the rail, and the rail is still the default. It no longer describes the only layout. The ladder, the floor, zoom, focus, and the keymap are shared; only the body branches.
- `1`/`2`/`3`/`4` mean different things in the two layouts — expansion in the rail, navigation in the table. That is the honest cost of 1c and is stated in the panel's own description of each mode.
- Every render test runs against both layouts, and the no-panic size sweep covers both.
- The table layout needs data the rail never asked for: `startedDate` for uptime and `memoryInBytes` for the memory ceiling. Both were already in the CLI's JSON.
- `BUSHEL_CONFIG_DIR` now overrides the config directory, so the settings panel's writes can be tested without touching a real dotfile.
- Not taken from 1c's 200×50 board: the structured image summary with layer history and a raw-JSON toggle. It needs data bushel does not fetch, and it is a separate decision.

# Issue tracker: local files

Tracker: local
Project: .scratch/
Adapter flags: none. The skills read and write the files below.
Auth: none.
Keys: a file path, like `.scratch/checkout/tickets/02-charge-card.md`.

## Conventions

One effort per directory: `.scratch/<slug>/`. Build tickets are one file each under `.scratch/<slug>/tickets/NN-<ticket-slug>.md`, numbered from `01`.

```markdown
# <title>

Status: ready-for-agent
Category: bug | enhancement
Builds toward: <map path, spec path, or the destination in a line>
Blocked by: 01-<ticket-slug>.md

<agent brief>
```

- **State** is the `Status:` line. The role names are the strings: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`, plus `claimed` and `closed`.
- **Next** is the lowest-numbered file with `Status: ready-for-agent` whose every `Blocked by:` file is `closed`.
- **Claim** is rewriting `Status: claimed` with your name on the next line, before any work.
- **Comment** is appending under a `## Comments` heading at the bottom, with a date.
- **Close** is `Status: closed`.

A pull request that resolves a local ticket names the file in its body. Nothing closes the file on merge; the builder does.

## Pull requests as a request surface

No.

## External authors

Not applicable.

## Wayfinding operations

The map is `.scratch/<slug>/map.md`. Tickets are `.scratch/<slug>/issues/NN-<slug>.md` with `Type:` and `Status:` lines. Blocking is a `Blocked by:` line. The frontier is the lowest-numbered open, unblocked, unclaimed file. Claim is `Status: claimed`. Resolve is the answer under `## Answer`, `Status: closed`, and one gist line on the map.

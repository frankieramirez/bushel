# Scratch fallback

`map.sh` exited 3. This token can read issues and often push branches. It cannot create or edit issues. Stop calling `gh issue create`.

Write the same map shape under `.scratch/<slug>/` in this checkout.

```
.scratch/<slug>/map.md
.scratch/<slug>/issues/01-<ticket-slug>.md
.scratch/<slug>/issues/02-<ticket-slug>.md
```

`<slug>` is a short kebab name for the destination.

## map.md

Use the body from `map-shape.md`. Add a status line at the top:

```markdown
# Map: <destination in a few words>

Status: open
Type: wayfinder:map
Tracker: local fallback. GitHub issue write returned 403 from this token.

## Destination
...
```

Under Decisions so far, link tickets as `issues/01-<slug>.md`.

## Ticket files

```markdown
# <title>

Status: open
Type: wayfinder:<type>
Part of: ../map.md
Blocked by:

## Question

<the decision or investigation>
```

Claim is rewriting `Status: claimed` and putting your name under it. Resolve is `Status: closed`, the answer at the bottom, and a gist line on the map.

Tell the user the map is local because GitHub refused the write. They can publish it later with a token that can create issues. Do not write a publisher unless they ask.

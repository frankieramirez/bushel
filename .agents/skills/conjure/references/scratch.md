# Scratch fallback

`tickets.sh` exited 3. This token can read issues and often push branches. It cannot create or edit issues. Stop calling `gh issue create`.

Write the same tickets under `.scratch/<slug>/tickets/` in this checkout. `<slug>` is a short kebab name for the destination.

```
.scratch/<slug>/tickets/01-<ticket-slug>.md
.scratch/<slug>/tickets/02-<ticket-slug>.md
```

Each file:

```markdown
# <title>

Status: ready-for-agent
Category: bug | enhancement
Builds toward: <map url, spec path, or the destination line>
Blocked by: 01-<ticket-slug>.md

<brief from agent-brief.md>
```

A build session takes a file the same way it takes an issue: the path is the ticket, the brief is the contract, and claiming is rewriting `Status: claimed` with your name under it.

Tell the user the tickets are local because GitHub refused the write. They can file them later with a token that can create issues. Do not write a publisher unless they ask.

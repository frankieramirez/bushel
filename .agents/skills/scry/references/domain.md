# Domain

Sharpen the project's language as decisions land. Reading `CONTEXT.md` for vocabulary is a habit. This file is for when a term or a hard-to-reverse choice actually changes.

## Where it lives

Most repos have one context: `CONTEXT.md` at the root, ADRs in `docs/adr/`.

If `CONTEXT-MAP.md` exists, it points at per-area `CONTEXT.md` files. Edit the one that owns the term.

Create files when you have something to write. The first resolved term creates `CONTEXT.md`. The first ADR creates `docs/adr/`.

## During grilling

When the user uses a term that conflicts with `CONTEXT.md`, say so immediately: "The glossary defines cancellation as X, and you just used it as Y. Which one holds?"

When a word is vague or overloaded, offer a precise canonical term and wait.

When a relationship is in play, invent a concrete scenario that forces the boundary into the open. Check the code when they state how something works. If the code disagrees, surface it.

## Write it down when it lands

Update `CONTEXT.md` in the same turn a term is resolved. The file is a glossary. Keep implementation detail out of it.

A term entry is a heading and a short definition in the project's own words:

```markdown
## Cancellation

Releasing an unfilled hold. Partial cancellation is allowed; the remaining hold stays open.
```

Offer an ADR only when all of these hold: the choice is hard to reverse, a future reader will wonder why it went this way, and real alternatives were on the table. Skip the ADR when any of those is missing.

```markdown
# <number>. <title>

Date: <YYYY-MM-DD>

## Context

<the situation that forced a choice>

## Decision

<what we picked>

## Consequences

<what this makes easier, and what it costs>
```

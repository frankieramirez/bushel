# Map shape

The map is one GitHub issue labelled `scry:map`. Its tickets are child issues.

## Map body

Open tickets are omitted from the body. They are children, found by `map.sh frontier`.

```markdown
## Destination

<what reaching the end looks like: the spec, decision, or change. One or two lines.>

## Notes

<domain; files every session should read; standing preferences for this effort; owning doc if resolutions should land somewhere besides the ticket>

## Decisions so far

<!-- one line per closed ticket: gist, then the link for the detail -->

## Not yet specified

<!-- in-scope fog you cannot ticket yet; graduates as the frontier advances -->

## Out of scope

<!-- work ruled past this destination; closed, never graduates -->
```

## Tickets

Each child is one question, sized to one session. The tracker's issue number is its identity.

```markdown
## Question

<the decision or investigation this ticket resolves>
```

Label with exactly one of `scry:research`, `scry:prototype`, `scry:grilling`, `scry:task`.

The answer is written on resolution (a comment), never as part of the filed body.

## Ticket types

Every ticket is **HITL** (worked with a human who speaks for themselves) or **AFK** (the agent drives it alone). A HITL ticket only resolves through that live exchange. Answering your own grilling questions has broken this, unless the user passed `you-pick`.

- **Research** (AFK). A fact a later decision waits on, found in docs, APIs, or other primary sources outside the current working directory. Resolved by a subagent following `research.md`.
- **Prototype** (HITL). A cheap, rough artifact to react to: an outline, a stub, a UI or logic sketch. Use when the question is how something should look or behave. Link the artifact from the ticket.
- **Grilling** (HITL). Conversation. The default. Load `grilling.md` and `domain.md`.
- **Task** (HITL or AFK). Manual work that must happen before a decision can be made: signing up for a service, provisioning access, moving data so its shape can be seen. This type does work so a decision can proceed. It does not deliver the destination. The agent drives it alone where it can; otherwise it hands the human a checklist. The answer records what was done and any facts later tickets need (URLs, row counts, where credentials landed).

A defect (something broken) is a regular GitHub issue, labelled from `docs/agents/triage-labels.md` when that file exists (`needs-triage` by default). It does not join the map.

## Fog

The map stays incomplete on purpose. Beyond the live tickets is fog: decisions you can tell are coming and cannot yet pin down.

**Not yet specified** holds that dim view. Everything there is in scope and too coarse to ticket. Write it as loosely as the view allows.

**Ticket when** the question is already sharp, even if it is blocked.
**Not yet specified when** you cannot yet phrase it that sharply. One patch of fog may become several tickets later, or none.

**Not yet specified** excludes Decisions so far, live tickets, and Out of scope.

## Out of scope

The destination fixes the scope. Work beyond it belongs in **Out of scope**.

Out-of-scope work never graduates. It returns only if the destination is redrawn, as a fresh effort.

When a ticket already on the map turns out to sit past the destination, close it and leave one line in Out of scope: the gist, why, and a link. Leave it out of Decisions so far. A scope boundary is not a step on the route.

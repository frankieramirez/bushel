# Slicing

Cut the destination into tickets a single build session can finish and a reviewer can judge on its own.

## What a slice is

- **Vertical.** Each slice makes one observable thing true end to end: a request that returns, a command that runs, a screen that renders. A slice that only adds a type, a table, or a helper is a layer. Layers hide inside the slice that first needs them.
- **One session.** If you cannot see the whole diff from here, it is two slices. Split along an interface and wire the edge.
- **Walking skeleton first.** The first ticket is the thinnest path through every part the destination touches, even if every part is a stub. Everything after it thickens one part.
- **Independently checkable.** Every acceptance criterion is something a caller or user can see, with the test or command that shows it. "Refactor the loader" is a task, not a criterion.
- **Interfaces, not paths.** Name the type, the function signature, the route, the flag. Files move between filing and building.
- **Out of scope on every ticket.** Adjacent work that a builder would otherwise drift into. Name it so it stays off this ticket and lands on its own.

## Edges

A ticket blocks another only when the second cannot be built without the first's interface in place. Sharing a file is no reason for an edge. Keep the graph shallow; a chain of six is a sign the slices are layers.

## The round

Present every slice at once, numbered, with its summary and the tickets it waits on. Give a recommended order. Then wait.

```
**1. <title>**
<one line: what is true when it closes>
After: <numbers, or none>

Recommended order: 1, 2, 3 and 4 in parallel, then 5
```

`you-pick`, or the user saying "make the decisions" or "you pick", accepts every recommendation. Still show the slices and the order you took, so they can override.

Fold the answers back in. A merged or split slice gets a new brief. Do not file until the round is settled.

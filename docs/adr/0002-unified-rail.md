# 0002 — Unified rail at every size

## Status

Accepted (2026-08-25, [Prototype: the unified rail ladder](https://github.com/frankieramirez/bushel/issues/15), wayfinder map [bushel responsive visual UI](https://github.com/frankieramirez/bushel/issues/14))

## Context

v0.1 shows one pane at a time, 45/55 list/detail. Daily use made two complaints: a ~60-column detail pane leaves logs unreadable, and a 200-column terminal wastes most of its width on the list. A parked stash implemented stacking below 90 columns and a three-panel rail only above 150, which is a tabs-then-rail mode split.

## Decision

One model at every size. The left (or top) column is **always the rail**: all three panes present, inactive ones collapse, the active panel takes the flexible space. The ladder only places the rail: **beside** the detail pane when body width is 80 or more, **above** it when narrower. The rail never grows past **36** columns; spare width belongs to logs.

`1`/`2`/`3` expand that pane rather than replacing the others. Zoom still fullscreens the active panel's table, not the whole rail. Header is a pane switcher without counts; counts live on the rail.

This is a real trade-off against keeping v0.1 tabs at small sizes and growing the list with the terminal. Tabs-then-rail would mean two interaction models. A 45% list at 200 columns spends the extra width on names instead of logs, which is the opposite of the complaint.

## Consequences

- **Pane** no longer means "one view at a time." Glossary and SPEC follow this ADR.
- Floor chrome (~55×20): 1-row header, no table headers, no Logs/Inspect tab row (`l`/`i` still work), no status cluster.
- Tight collapse (stacked, or rail height under 16) is a 1-row `2 images 5`. Roomy collapse is shrink-to-fit names, cap `max(8, height/4)`.
- Overlay floor behavior is a separate decision ([Grilling: overlay behavior at the 55×20 floor](https://github.com/frankieramirez/bushel/issues/20)): the action menu covers the detail pane only.
- Throwaway reference: [`prototype/unified-rail-ladder`](https://github.com/frankieramirez/bushel/tree/prototype/unified-rail-ladder/prototype-rail). Steal look, not code.

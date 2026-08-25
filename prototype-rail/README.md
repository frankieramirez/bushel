# PROTOTYPE — unified rail ladder

Throwaway Ratatui prototype for [Prototype: the unified rail ladder](https://github.com/frankieramirez/bushel/issues/15).
Fake data only — never touches the `container` CLI. Do not merge to main; the branch is the artifact.

The layout model is the one settled at charting: **the rail is always present** (all three panels; inactive ones collapse). The ladder only decides whether the rail sits **beside** the detail pane or **above** it.

## Run

```sh
cd prototype-rail
cargo run                 # live terminal; resize freely
cargo run -- --dump       # print 55×20 / 100×30 / 200×50 frames to dumps/
```

Size presets (the three reaction tiers) without resizing the window:

- `F1` — 55×20 (the floor)
- `F2` — 100×30 (medium)
- `F3` — 200×50 (wide)
- `F4` — live (fill the real terminal)

The preset is a centered viewport; prototype chrome around it shows the layout numbers. `F4` is the honest resize test.

## Starting numbers (react to these)

| Knob | Starting value | Why |
| --- | --- | --- |
| Stack breakpoint | body width **< 80** → rail above | 55 must stack; 100 should sit beside |
| Rail width cap | **36** columns | spare width belongs to logs; at 100 cols this is what actually gets logs over 60 |
| Tight collapse | 1-row `2 images 5` when stacked or rail height < 16 | 55×20 cannot afford boxed inactive panels |
| Roomy collapse | shrink-to-fit, cap `height/4` | glanceable names, not a second manager |
| 55×20 drops | 1-row header, no table headers, no Logs/Inspect tab row, compact hints | otherwise the active list and logs both die |

## Keys (app)

`1` `2` `3` expand that panel · `Tab` cycle · `j`/`k` move · `Enter`/`Esc` focus · `/` filter · `f` zoom · `l`/`i` logs/inspect · `q` quit · `?` help

## What to react to

1. **Breakpoint** — is 80 the right place for the rail to climb on top of the logs? Try F1 vs F2 vs a live resize through ~70–90.
2. **Inactive collapse** — does title+count (tight) / shrink-to-fit (roomy) read, or do inactive panels need a row or two of names at the floor too?
3. **Width cap** — at F2 logs should clear 60 cols; at F3 they get the rest. Is 36 too greedy or too shy?
4. **`1`/`2`/`3` as expand** — does expanding a panel (rather than replacing the others) feel like the same app at every size? Filter and per-panel selection memory ride along.
5. **The 55×20 floor** — is dropping table headers, the detail-tab row, and a header line acceptable? What else has to go?

Static dumps from `--dump` live in `dumps/` on this branch as a fallback; the real verdict needs a terminal.

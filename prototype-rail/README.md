# PROTOTYPE — telemetry strip

Throwaway Ratatui prototype for [Prototype: the telemetry strip](https://github.com/frankieramirez/bushel/issues/17).
Fake data only. Hosted in the locked unified-rail layout so the 55×20 collapse is a real constraint. Do not merge to main; the branch is the artifact.

Research locked on [Research: telemetry rendering in TUIs and Ratatui's chart toolkit](https://github.com/frankieramirez/bushel/issues/16): Sparkline eighth-blocks, no braille, newest-first + RightToLeft, numbers for net/disk, sparklines for cpu/mem.

## Run

```sh
cd prototype-rail
cargo run                 # live terminal; history ticks
cargo run -- --ascii      # ASCII bar set from the start
cargo run -- --dump       # rewrite dumps/
```

Size presets: `F1` 55×20 · `F2` 100×30 · `F3` 200×50 · `F4` live.

Strip keys: `s` cycles 2 / 3 / 4 rows · `a` toggles ASCII · `l`/`i` logs vs inspect.

## Starting numbers (react to these)

| Knob | Starting value |
| --- | --- |
| Height | **3 rows**: cpu spark, mem spark, net+disk as one text row |
| Glyphs | eighth-block `▁▂▃▄▅▆▇█`; ASCII ramp ` .:-=+*#`; sparks auto-scale, number stays true % |
| Collapse | strip yields when detail inner < strip_h + **4** log rows |
| Window | 5 minutes at 1s; sparkline shows the most recent **width** seconds, newest on the right |
| Inspect | same strip as Logs |

## What to react to

1. **Height / layout** — 3-row default. `s` tries 2-row (cpu|mem side by side) and 4-row (rates get sparklines too). Clutter or too shy?
2. **Glyphs** — eighth-blocks vs `a` ASCII. Does the fallback still read as a sparkline?
3. **Collapse** — at F1 a 3-row strip just fits; `s` to 4-row should collapse and give logs the pane. Right threshold?
4. **Logs vs Inspect** — same strip on both. Does Inspect want it, or only Logs?
5. **Five minutes** — one second per column, so a 60-col pane shows one minute, not five. Honest, or do we need a denser glyph?

The rail numbers are already locked (stack below 80, cap 36, tight/roomy collapse) and are not up for re-litigation here.

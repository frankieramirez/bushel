# 0001 — Motion-first TUI, bounded by hard rules

## Status

Accepted (2026-08-19, UX-skeleton grilling, wayfinder ticket #5)

## Context

bushel's stated ambition is to improve on lazydocker/lazygit/k9s, not copy them. Those tools are static; the reference point for the feel we want is oh-my-pi's launch and transition polish. Ratatui's immediate-mode render loop plus the `tachyonfx` effects crate make terminal animation genuinely feasible. The alternative was the classic static TUI: cheaper to build, zero redraw cost, but indistinguishable from the prior art.

## Decision

bushel commits to a motion language as a baseline feature, not decoration:

- **Splash-as-loading**: an animated bushel mark plays only while the startup probes (`system status`, initial `ls`) run, dissolving into the layout when data arrives. Any key skips; `--no-splash` and a config option disable it.
- **Targeted micro-motion**: pane-switch slide/fade, modal fade-in, focus-border glow, action-menu bottom-sheet slide, toast slide-in, smooth scroll easing, poll-tick spinner.
- **Hard rules**: every animation ≤150ms, interruptible by input, and never delays data display or input handling. A `reduced-motion` config disables all of it.
- **Ambient effects** (gradient drift, idle flourishes) are prototype-gated: one is built in the layout prototype and kept only if it doesn't read as noise or cost meaningful CPU/battery.

## Consequences

- The render loop must run at animation framerate while effects are live and drop to poll-tick cadence when idle; this shapes the event-loop architecture (ticket #6).
- `tachyonfx` (or equivalent) becomes a core dependency.
- Terminal capability detection (truecolor → 256-color, Nerd Font → ASCII) is required, since the aesthetic direction (dark-first, gradient accents, rounded borders) assumes modern terminals but must degrade.
- The ≤150ms/interruptible/reduced-motion rules are the contract that keeps this from becoming the thing users disable; any future animation that can't meet them doesn't ship.

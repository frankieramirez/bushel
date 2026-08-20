# Contributing to bushel

Thanks for your interest. bushel is built primarily for the author's own workflow, so scope is guarded deliberately — read [SPEC.md](SPEC.md) (especially the non-goals) before proposing features.

## Ground rules

- **Bugs**: open an issue with your `container --version`, macOS version, and the relevant lines from bushel's message log (`m`).
- **Features**: open an issue first. Anything on the SPEC.md non-goals list needs a strong case; PRs for undiscussed features may be declined.
- **Vocabulary**: [CONTEXT.md](CONTEXT.md) defines the project's terms (and the words we avoid). Code, issues, and docs should use them.

## Pull requests

- Keep the layering: `runner` → `client` → `engine` → `ui`. Version-fragile parsing belongs in the Client; the Engine stays headless.
- `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` must pass.
- New Client parsing needs fixture coverage; new Engine behavior needs a headless test.
- Any animation must obey ADR 0001's hard rules: ≤150ms, interruptible, disabled by `reduced-motion`.

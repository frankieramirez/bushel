# Contributing to bushel

Thanks for your interest. bushel is built primarily for the author's own workflow, so scope is guarded deliberately — read [SPEC.md](SPEC.md) (especially the non-goals) before proposing features.

## Where to ask or report

| Kind | Where |
| --- | --- |
| Question / install help / "how do I…" | [Discussions → Q&A](https://github.com/frankieramirez/bushel/discussions/new?category=q-a) |
| Bug | [Bug report](https://github.com/frankieramirez/bushel/issues/new?template=bug_report.md) |
| Feature proposal | [Feature request](https://github.com/frankieramirez/bushel/issues/new?template=feature_request.md) (check [the roadmap](https://github.com/frankieramirez/bushel/issues/56) first) |
| Security vulnerability | [SECURITY.md](SECURITY.md) — not a public issue |

Also skim [bushel.sh/docs/troubleshooting](https://bushel.sh/docs/troubleshooting) before opening either path.

## Ground rules

- **Bugs**: include your `container --version`, macOS version, bushel version, and the relevant lines from bushel's message log (`m`).
- **Features**: open an issue first. Anything on the SPEC.md non-goals list needs a strong case; PRs for undiscussed features may be declined.
- **Vocabulary**: [CONTEXT.md](CONTEXT.md) defines the project's terms (and the words we avoid). Code, issues, discussions, and docs should use them.
- **Conduct**: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Pull requests

- Keep the layering: `runner` → `client` → `engine` → `ui`. Version-fragile parsing belongs in the Client; the Engine stays headless.
- `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` must pass.
- New Client parsing needs fixture coverage; new Engine behavior needs a headless test.
- Any animation must obey ADR 0001's hard rules: ≤150ms, interruptible, disabled by `reduced-motion`.
- New keybindings go in `src/ui/help.rs` — the cheatsheet is the only keymap the docs read, and tests fail both ways if a binding and its row disagree.
- New config options need a matching `--flag` on `bushel::cli::Args`; `docs.json` pairs them by name and refuses to build if one is missing.

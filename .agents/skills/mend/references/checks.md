# Checks

Run the checks the project already has. Discover, then run: typecheck, then tests, then format. Add lint when the project names a lint script.

## Discover

Look in this order and stop adding once you have a command for a category. Do not invent a script name.

1. A `Validation:` line in the `## Agent skills` block of `CLAUDE.md` or `AGENTS.md`. When it exists, that command is the suite; the rest of this list only fills in typecheck and format.
2. `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `README.md` (sections named test, check, ci, verify)
3. `.github/workflows/*` (the jobs that run on pull_request)
4. `package.json` scripts named `typecheck`, `tsc`, `test`, `lint`, `format`, `check`
5. `Makefile` / `justfile` targets with those names
6. Language defaults only when nothing above exists: `cargo test`, `go test ./...`, `pytest`, `python -m compileall` as they apply

A format command that rewrites the whole tree is fine after a merge. A lint command that fails on pre-existing issues outside the conflicted files: note it and continue if you can tell the failures are old.

## Run

Typecheck first. Then the tests that cover the conflicted files. Then the project's broader suite if it is cheap (under a couple of minutes) or if the docs say to run it after a merge. Format last so the commit is clean.

Fix failures the merge caused. A failure that already existed on `HEAD` before the operation: record it under Open and leave it.

## What "the merge broke" looks like

- A symbol one side renamed and the other side still calls
- An import that points at a file the other side moved
- A test that asserts the old shape of something both sides changed
- A lockfile that no longer matches the manifest

Fix those. Do not use a failing test as a chance to redesign the feature.

# Out of scope files

`.out-of-scope/` holds rejected enhancements so the reason survives the closed issue and a later request can be matched against it.

One file per concept, kebab-case (`dark-mode.md`). Several issues asking for the same thing share a file.

Write here only when an **enhancement** is rejected as `wontfix`. A bug rejection does not get a file. An issue closed because the feature already exists does not get a file either: point at the code in the closing comment.

## File shape

```markdown
# Dark mode

This project does not ship user-facing theming.

## Why

The rendering pipeline assumes one palette in `ThemeConfig`, resolved at build time. Runtime switching would need a theme context around the tree and a place to persist a preference. That sits outside what this repo is for. Downstream consumers who embed the output can theme there.

## Prior requests

- #42: "Add dark mode support"
- #87: "Night theme for accessibility"
```

The reason should still make sense in a year. "We are busy" is a deferral. Leave those issues open under `needs-triage` or move them to a later state the maintainer names.

## When to read

During gather, read every `.out-of-scope/*.md`. Match by concept ("night theme" matches `dark-mode.md`). If one matches, show the maintainer the file and the reason. They confirm (append this issue to Prior requests, close), reconsider (delete or edit the file, continue sifting), or split (related idea, distinct request: continue).

## When to write

1. Maintainer rejects an enhancement.
2. Reuse a matching file if one exists; otherwise create it.
3. Append the issue to Prior requests.
4. Comment with the decision and the file path, then close as `wontfix`.

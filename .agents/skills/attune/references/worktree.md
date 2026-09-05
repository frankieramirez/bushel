# Worktree files

Load at Stage 3 for the `worktree` setting.

Every Orca worktree is a fresh checkout without gitignored files, so a validation command that needs `.env` or `node_modules` fails there until someone sets them up. Two files at the repo root fix that, and Orca reads both. Skip the whole setting when the validation command needs neither.

## `.worktreeinclude`

One gitignored file or directory per line, copied into each new worktree.

```
.env
.env.local
```

Small files only. A copied `node_modules` stalls worktree creation, and that is what `orca.yaml` is for instead.

## `orca.yaml`

```yaml
scripts:
  setup: pnpm install
worktree:
  sharedDirectories:
    - node_modules
    - .cache
```

`scripts.setup` holds the install command a fresh checkout needs. `worktree.sharedDirectories` lists large rebuildable gitignored directories that already exist in the primary checkout, so a new worktree shares them rather than rebuilding them.

## Editing rules

An existing file is edited key by key and never replaced. A path already listed stays listed. Show the draft before writing either file, since both change how every future worktree is built.

A directory that does not exist in the primary checkout does not belong in `sharedDirectories`: Orca has nothing to share, and the worktree gets an empty directory that looks installed.

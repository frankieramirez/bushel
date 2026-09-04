# Release checklist

Ship one semver (`Cargo.toml` / git tag `vX.Y.Z`) to every channel in the same cut.

1. Tag `vX.Y.Z` and publish the GitHub Release (binaries and the update channel).
2. Bump the Homebrew tap to that version.
3. Publish the same version to crates.io (secondary).

`cargo install bushel` is a secondary install path. Primary remains the tap, the curl installer, and `bushel update`.

`bushel update` (axoupdater) keeps targeting GitHub Releases. crates.io does not replace that.

## crates.io

Release CI calls `.github/workflows/publish-crates.yml` after the GitHub Release is announced (`post-announce-jobs` in `dist-workspace.toml`). It publishes the same `Cargo.toml` version as the tag.

Automation needs a `CARGO_REGISTRY_TOKEN` repository secret (a crates.io API token). Do not put the token in the repo, in workflow files, or in chat.

If that secret is unset, the job succeeds and skips the upload so brew / curl / `bushel update` still ship. Then publish the same version by hand:

```sh
cargo publish
```

CI on every PR runs `cargo publish --dry-run`, which does not need a token.

README documents `cargo install bushel` as the secondary command. The `--git` fallback is gone now that crates.io publish is wired.

A crates.io-only prerelease without a matching tag stays out, as does yank-as-update. The publish job also skips cargo-dist prerelease tags.

# Release checklist

Ship one semver (`Cargo.toml` / git tag `vX.Y.Z`) to every channel in the same cut.

1. Tag `vX.Y.Z` and publish the GitHub Release (binaries and the update channel).
2. Bump the Homebrew tap to that version.
3. `cargo publish` the same version to crates.io.

`cargo install bushel` is a secondary install path. Primary remains the tap, the curl installer, and `bushel update`.

`bushel update` (axoupdater) keeps targeting GitHub Releases. crates.io does not replace that.

Leave the README `cargo install --git` wording until the first crates.io publish lands, then switch it to `cargo install bushel` as secondary.

A crates.io-only prerelease without a matching tag stays out, as does yank-as-update.

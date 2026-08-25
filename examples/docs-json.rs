//! Emits `docs.json` for bushel.sh — `cargo run --example docs-json -- --out docs.json`,
//! or to stdout with no argument.
//!
//! An example rather than a `[[bin]]` on purpose: the release ships every bin
//! it builds, so a second one would land in every user's PATH (and in the
//! Homebrew formula) to serve a website. Examples are never installed by
//! `cargo install`, never packaged by `dist`, and still compiled by
//! `cargo test` and `cargo clippy --all-targets`, so this cannot rot.

use std::io::Write as _;

fn main() -> std::io::Result<()> {
    let json = bushel::docs::to_json().map_err(std::io::Error::other)?;
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--out") => {
            let Some(path) = args.next() else {
                eprintln!("docs-json: --out needs a path");
                std::process::exit(2);
            };
            std::fs::write(&path, json)?;
            eprintln!("docs-json: wrote {path}");
        }
        Some(other) => {
            eprintln!("docs-json: unknown argument `{other}` (expected --out <path>)");
            std::process::exit(2);
        }
        None => std::io::stdout().write_all(json.as_bytes())?,
    }
    Ok(())
}

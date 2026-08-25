//! The README's keymap and options are generated, and this test is what keeps
//! them that way.
//!
//! The hand-copied keymap in the README had drifted four bindings behind the
//! cheatsheet by the time the docs site was charted, which is why nothing in
//! this repo hand-maintains a second copy any more. A bot that rewrote the
//! README on every push was the other option and it is worse: it churns commits,
//! it fights whoever is editing the file, and it hides the drift instead of
//! reporting it. This fails `cargo test` — which CI already runs — and prints
//! the fix.
//!
//! Fixing drift is one command:
//!
//! ```sh
//! UPDATE_README=1 cargo test --test readme
//! ```

use std::path::PathBuf;

/// A generated region of the README: everything between these two comments.
struct Block {
    name: &'static str,
    start: &'static str,
    end: &'static str,
    /// What the source says the region should contain.
    expected: String,
}

fn readme_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("README.md")
}

fn blocks() -> Vec<Block> {
    vec![
        Block {
            name: "keys",
            start: "<!-- keys:start -->",
            end: "<!-- keys:end -->",
            expected: bushel::docs::keys_markdown(),
        },
        Block {
            name: "options",
            start: "<!-- options:start -->",
            end: "<!-- options:end -->",
            expected: bushel::docs::options_markdown().expect("every flag pairs with a config key"),
        },
    ]
}

/// The region between the markers, as it should read: blank line, content, then
/// the closing marker on its own line.
fn rendered(block: &Block) -> String {
    format!("\n\n{}\n", block.expected.trim_end())
}

/// Byte range of the text between the two markers, or a panic naming what is
/// missing — a README without the markers cannot be checked at all.
fn between(readme: &str, block: &Block) -> std::ops::Range<usize> {
    let start = readme
        .find(block.start)
        .unwrap_or_else(|| panic!("README.md has no `{}` marker", block.start))
        + block.start.len();
    let end = readme[start..].find(block.end).unwrap_or_else(|| {
        panic!(
            "README.md has no `{}` marker after `{}`",
            block.end, block.start
        )
    }) + start;
    start..end
}

#[test]
fn the_readme_matches_what_bushel_generates() {
    let path = readme_path();
    let mut readme = std::fs::read_to_string(&path).expect("README.md is readable");
    let update = std::env::var_os("UPDATE_README").is_some();

    let mut stale = Vec::new();
    // Back to front, so an earlier replacement cannot shift a later range.
    for block in blocks().iter().rev() {
        let range = between(&readme, block);
        let want = rendered(block);
        if readme[range.clone()] == want {
            continue;
        }
        stale.push(block.name);
        readme.replace_range(range, &want);
    }

    if stale.is_empty() {
        return;
    }

    if update {
        std::fs::write(&path, &readme).expect("README.md is writable");
        return;
    }

    stale.reverse();
    panic!(
        "README.md is behind bushel's own keymap/flags ({}).\n\
         Regenerate it with:\n\n    UPDATE_README=1 cargo test --test readme\n\n\
         The README it should be:\n\n{readme}",
        stale.join(", ")
    );
}

/// The generated block cannot carry `?`: it opens the cheatsheet and the
/// cheatsheet does not list itself. The README says so in prose instead, which
/// leaves it the one line about keys still hand-written. If `?` ever joins the
/// cheatsheet, that line becomes a second copy and should go.
///
/// The other half of this — that `?` still opens the help overlay at all —
/// is `ui::keymap`'s to hold, next to the sweep that exempts it.
#[test]
fn the_generated_block_leaves_the_question_mark_to_prose() {
    let keys = bushel::docs::keys_markdown();
    assert!(
        !keys.contains('?'),
        "`?` is on the cheatsheet now, so the README's hand-written line about it is a second copy"
    );
}

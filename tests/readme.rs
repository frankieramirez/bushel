use std::path::PathBuf;

struct Block {
    name: &'static str,
    start: &'static str,
    end: &'static str,
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

fn rendered(block: &Block) -> String {
    format!("\n\n{}\n", block.expected.trim_end())
}

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

#[test]
fn the_generated_block_leaves_the_question_mark_to_prose() {
    let keys = bushel::docs::keys_markdown();
    assert!(
        !keys.contains('?'),
        "`?` is on the cheatsheet now, so the README's hand-written line about it is a second copy"
    );
}

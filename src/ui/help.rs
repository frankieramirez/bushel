//! The cheatsheet — the one place that says what bushel's keys do.
//! `draw` renders it into the help overlay; `docs` serialises it for the
//! website. Nothing else may hold a second copy of the keymap: the README's
//! copy had already drifted four bindings behind this list when the docs site
//! was charted.

/// One row of the cheatsheet: a group heading (`keys` empty) or a binding.
pub struct HelpRow {
    pub keys: &'static str,
    pub desc: &'static str,
}

const fn head(desc: &'static str) -> HelpRow {
    HelpRow { keys: "", desc }
}
const fn bind(keys: &'static str, desc: &'static str) -> HelpRow {
    HelpRow { keys, desc }
}

/// One cheatsheet at every size — no shorter floor variant.
pub const HELP: &[HelpRow] = &[
    head(" global"),
    bind("1/2/3, tab", "expand pane (containers / images / volumes)"),
    bind("f", "zoom focused side"),
    bind("m", "message log"),
    bind("b", "dismiss version banner"),
    bind("q", "quit"),
    head(" list"),
    bind("j/k g/G", "move / top / bottom"),
    bind("/", "fuzzy filter (esc clears)"),
    bind("enter", "focus detail pane"),
    bind("space", "action menu"),
    bind(
        "s r K d P e",
        "start/stop · restart · kill · delete · prune · exec",
    ),
    bind("u", "pull image (images pane)"),
    head(" detail"),
    bind("l / i", "logs / inspect tab (containers)"),
    bind("F", "toggle follow"),
    bind("w", "toggle wrap / truncated"),
    bind("pgup/pgdn", "scroll without switching focus"),
    bind("esc", "back to list"),
];

/// Width of the key column, including its leading pad.
pub const HELP_KEY_COL: u16 = 14;

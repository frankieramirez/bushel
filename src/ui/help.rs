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

pub const HELP: &[HelpRow] = &[
    head(" global"),
    bind(
        "1/2/3/4, tab",
        "expand pane (containers / images / volumes / networks)",
    ),
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

pub const HELP_KEY_COL: u16 = 14;

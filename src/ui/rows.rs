use ratatui::style::Style;
use ratatui::text::Span;

use crate::engine::state::ContainerEntry;
use crate::ui::humanize::{age_now, collapse_reference, elide, uptime_now};
use crate::ui::theme::Theme;

pub const SELECT_BAR: &str = "▎";
pub const SELECT_BAR_ASCII: &str = "|";

pub fn select_bar(th: &Theme) -> &'static str {
    if th.ascii {
        SELECT_BAR_ASCII
    } else {
        SELECT_BAR
    }
}

pub const ABSENT: &str = "·";
pub const ABSENT_ASCII: &str = "-";

pub fn absent(th: &Theme) -> &'static str {
    if th.ascii { ABSENT_ASCII } else { ABSENT }
}

/// First row of a window `room` rows tall that still contains `selected`.
pub fn scroll_start(selected: Option<usize>, room: usize) -> usize {
    match selected {
        Some(sel) => sel.saturating_sub(room.saturating_sub(1)),
        None => 0,
    }
}

pub fn state_dot(th: &Theme, running: bool) -> Span<'static> {
    if running {
        Span::styled(th.dot_running(), Style::new().fg(th.accent()))
    } else {
        Span::styled(th.dot_stopped(), Style::new().fg(th.dim()))
    }
}

pub fn cpu_cell(th: &Theme, c: &ContainerEntry) -> String {
    match c.cpu_percent.filter(|_| c.is_running()) {
        Some(v) => format!("{v:.1}%"),
        None => absent(th).into(),
    }
}

/// `512M` — what the container is using right now.
pub fn mem_cell(th: &Theme, c: &ContainerEntry) -> String {
    match c.mem_bytes.filter(|_| c.is_running()) {
        Some(v) => compact_bytes(v),
        None => absent(th).into(),
    }
}

/// `512M / 2.0G` — usage against the ceiling, when the ceiling is known.
pub fn mem_of_limit(th: &Theme, c: &ContainerEntry) -> String {
    let used = mem_cell(th, c);
    match c.mem_limit {
        Some(limit) if limit > 0 && c.is_running() => {
            format!("{used} / {}", compact_bytes(limit))
        }
        _ => used,
    }
}

/// Bytes in one short token: `48M`, `1.4G`, `931K`.
pub fn compact_bytes(bytes: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1 << 30, "G"), (1 << 20, "M"), (1 << 10, "K")];
    for (scale, suffix) in UNITS {
        if bytes >= scale {
            let v = bytes as f64 / scale as f64;
            return if v < 10.0 {
                format!("{v:.1}{suffix}")
            } else {
                format!("{v:.0}{suffix}")
            };
        }
    }
    format!("{bytes}B")
}

pub fn uptime_cell(th: &Theme, c: &ContainerEntry) -> String {
    if c.is_running() {
        uptime_now(c.started.as_deref()).unwrap_or_else(|| absent(th).into())
    } else {
        age_now(c.started.as_deref()).unwrap_or_else(|| absent(th).into())
    }
}

pub fn age_cell(th: &Theme, created: Option<&str>) -> String {
    age_now(created).unwrap_or_else(|| absent(th).into())
}

/// An image reference as it should read in a list: registry host collapsed to a
/// dim two-letter token, name bright, tag dim.
///
/// `width` is the room the name and tag have *after* the token, so a caller can
/// reserve a fixed token column and keep every name starting at one character.
pub fn reference_spans(th: &Theme, reference: &str, width: usize) -> Vec<Span<'static>> {
    let (token, rest) = collapse_reference(reference);
    let rest = elide(&rest, width);
    let (name, tag) = split_tag(&rest);
    let mut spans = vec![Span::styled(
        format!("{token:<3}"),
        Style::new().fg(th.dim()),
    )];
    spans.push(Span::styled(name.to_string(), Style::new().fg(th.text())));
    if !tag.is_empty() {
        spans.push(Span::styled(tag.to_string(), Style::new().fg(th.dim())));
    }
    spans
}

/// Split `python:3.12-slim` into (`python`, `:3.12-slim`).
///
/// Only a colon after the last slash is a tag — `localhost:5000/app` has a port.
fn split_tag(reference: &str) -> (&str, &str) {
    let start = reference.rfind('/').map(|i| i + 1).unwrap_or(0);
    match reference[start..].rfind(':') {
        Some(i) => reference.split_at(start + i),
        None => (reference, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn theme() -> Theme {
        Theme {
            truecolor: false,
            ascii: false,
        }
    }

    fn container(running: bool) -> ContainerEntry {
        ContainerEntry {
            id: "web".into(),
            image: "alpine:latest".into(),
            state: if running { "running" } else { "stopped" }.into(),
            created: None,
            started: None,
            cpus: None,
            mem_limit: Some(2 * (1 << 30)),
            volumes: vec![],
            networks: vec![],
            cpu_percent: Some(14.1),
            mem_bytes: Some(512 * (1 << 20)),
            telemetry: VecDeque::new(),
            pending: None,
        }
    }

    #[test]
    fn the_window_scrolls_only_far_enough_to_hold_the_selection() {
        assert_eq!(scroll_start(Some(0), 5), 0);
        assert_eq!(scroll_start(Some(3), 5), 0, "still in view, do not scroll");
        assert_eq!(scroll_start(Some(4), 5), 0);
        assert_eq!(
            scroll_start(Some(9), 5),
            5,
            "the selection lands on the last row"
        );
        assert_eq!(scroll_start(None, 5), 0);
        assert_eq!(
            scroll_start(Some(9), 0),
            9,
            "a zero-row window cannot hide it"
        );
    }

    #[test]
    fn bytes_get_one_token_with_a_decimal_only_when_it_says_something() {
        assert_eq!(compact_bytes(512 * (1 << 20)), "512M");
        assert_eq!(compact_bytes(3 * (1 << 30) / 2), "1.5G");
        assert_eq!(compact_bytes(48 * (1 << 20)), "48M");
        assert_eq!(compact_bytes(900), "900B");
    }

    #[test]
    fn a_stopped_container_has_no_numbers_to_show() {
        let th = theme();
        let stopped = container(false);
        assert_eq!(cpu_cell(&th, &stopped), "·");
        assert_eq!(mem_cell(&th, &stopped), "·");
        assert_eq!(
            mem_of_limit(&th, &stopped),
            "·",
            "a ceiling nothing is running against is not information"
        );
    }

    #[test]
    fn a_running_container_shows_usage_against_its_ceiling() {
        let th = theme();
        let running = container(true);
        assert_eq!(cpu_cell(&th, &running), "14.1%");
        assert_eq!(mem_of_limit(&th, &running), "512M / 2.0G");

        let mut no_limit = container(true);
        no_limit.mem_limit = None;
        assert_eq!(mem_of_limit(&th, &no_limit), "512M");
    }

    #[test]
    fn ascii_mode_has_no_middle_dot_or_selection_bar() {
        let th = Theme {
            truecolor: false,
            ascii: true,
        };
        assert_eq!(absent(&th), "-");
        assert_eq!(select_bar(&th), "|");
        assert_eq!(cpu_cell(&th, &container(false)), "-");
    }

    fn rendered(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn a_reference_reads_token_then_name_then_tag() {
        let th = theme();
        let spans = reference_spans(&th, "docker.io/library/python:3.12-slim", 30);
        assert_eq!(rendered(&spans), "dh python:3.12-slim");
        assert_eq!(
            spans[0].content.as_ref(),
            "dh ",
            "the token column is fixed"
        );
        assert_eq!(spans[1].content.as_ref(), "python");
        assert_eq!(spans[2].content.as_ref(), ":3.12-slim");
    }

    #[test]
    fn a_bare_reference_still_lines_up_under_the_token_column() {
        let th = theme();
        let spans = reference_spans(&th, "comicarr:latest", 30);
        assert_eq!(rendered(&spans), "   comicarr:latest");
        assert_eq!(spans[0].content.as_ref(), "   ");
    }

    #[test]
    fn a_registry_port_is_not_a_tag() {
        assert_eq!(split_tag("localhost:5000/app"), ("localhost:5000/app", ""));
        assert_eq!(split_tag("app:1"), ("app", ":1"));
    }

    #[test]
    fn a_long_reference_is_elided_not_cut_off_at_the_head() {
        let th = theme();
        let spans = reference_spans(&th, "ghcr.io/blakeblackshear/frigate:stable", 16);
        let text = rendered(&spans);
        assert!(text.starts_with("gh "), "{text}");
        assert!(text.contains('…'), "{text}");
        assert!(
            text.ends_with("stable"),
            "the tag survives the elision: {text}"
        );
    }
}

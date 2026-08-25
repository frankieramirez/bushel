//! Logs-view layout: wrap vs truncated, and the mapping between a raw log
//! line and the display rows it occupies at the detail pane's width.

/// Split a raw log line into display rows.
///
/// Wrapped: as many rows as the pane width requires. Truncated: the line
/// unchanged (clip is visual; no ellipsis).
pub fn split_line(s: &str, wrap: bool, width: u16) -> Vec<String> {
    if !wrap || width == 0 {
        return vec![s.to_string()];
    }
    wrap_line(s, width)
}

/// Display-row index where raw line `idx` starts.
pub fn display_start(lines: &[String], wrap: bool, width: u16, idx: usize) -> u16 {
    let sum: usize = lines
        .iter()
        .take(idx)
        .map(|l| split_line(l, wrap, width).len())
        .sum();
    sum.min(u16::MAX as usize) as u16
}

/// Raw log line that owns display row `row`. Clamps to the last line.
pub fn raw_index(lines: &[String], wrap: bool, width: u16, row: u16) -> usize {
    if lines.is_empty() {
        return 0;
    }
    let mut acc: usize = 0;
    let row = row as usize;
    for (i, l) in lines.iter().enumerate() {
        let n = split_line(l, wrap, width).len();
        if row < acc.saturating_add(n) {
            return i;
        }
        acc = acc.saturating_add(n);
    }
    lines.len() - 1
}

/// Follow-tail display scroll: last `height` rows of `total`.
pub fn tail_scroll(total: u16, height: u16) -> u16 {
    total.saturating_sub(height)
}

/// Follow/pause marker copy from the wrap grilling.
pub fn follow_marker(follow: bool, wrap: bool) -> String {
    let state = if follow { "following" } else { "paused" };
    let mode = if wrap { "wrap" } else { "truncated" };
    format!("── {state} · {mode} (w) ──")
}

/// Bottom-bar hint: the mode `w` would switch *to*.
pub fn wrap_hint(wrap: bool) -> &'static str {
    if wrap { "unwrap" } else { "wrap" }
}

fn wrap_line(s: &str, width: u16) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    let mut rows = Vec::new();
    let mut current = String::new();
    let mut col = 0u16;
    for ch in s.chars() {
        let w = char_width(ch);
        if w == 0 {
            current.push(ch);
            continue;
        }
        if col > 0 && col.saturating_add(w) > width {
            rows.push(std::mem::take(&mut current));
            col = 0;
        }
        current.push(ch);
        if w > width {
            rows.push(std::mem::take(&mut current));
            col = 0;
        } else {
            col = col.saturating_add(w);
        }
    }
    if !current.is_empty() || rows.is_empty() {
        rows.push(current);
    }
    rows
}

fn char_width(ch: char) -> u16 {
    match ch {
        '\t' => 1,
        c if c.is_control() => 0,
        c if c.is_ascii() => 1,
        c => ratatui::text::Line::from(c.to_string()).width() as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_splits_a_long_line_to_the_pane_width() {
        assert_eq!(
            split_line("abcdefghij", true, 4),
            vec!["abcd", "efgh", "ij"]
        );
    }

    #[test]
    fn wrap_is_a_noop_for_short_lines() {
        assert_eq!(split_line("abc", true, 10), vec!["abc"]);
    }

    #[test]
    fn truncated_keeps_the_full_line_with_no_ellipsis() {
        let rows = split_line("hello world this is long", false, 4);
        assert_eq!(rows, vec!["hello world this is long"]);
        assert!(!rows[0].contains('…') && !rows[0].contains("..."));
    }

    #[test]
    fn empty_line_still_occupies_one_row() {
        assert_eq!(split_line("", true, 8), vec![""]);
        assert_eq!(split_line("", false, 8), vec![""]);
    }

    #[test]
    fn paused_toggle_keeps_the_same_raw_line_at_the_top() {
        let lines = vec![
            "aaaa".into(),
            "bbbbbbbbbb".into(), // 10 chars → 3 wrapped rows at width 4
            "cccc".into(),
        ];
        // wrapped display:
        // 0 aaaa          raw 0
        // 1 bbbb          raw 1
        // 2 bbbb          raw 1
        // 3 bb            raw 1
        // 4 cccc          raw 2
        let raw = raw_index(&lines, true, 4, 2);
        assert_eq!(raw, 1);
        assert_eq!(
            display_start(&lines, false, 4, raw),
            1,
            "truncated: the same raw line sits at the top"
        );
        assert_eq!(display_start(&lines, true, 4, raw), 1);
    }

    #[test]
    fn following_stays_on_the_tail() {
        // 5 raw lines, wrap off → 5 display rows; height 3 → scroll 2
        assert_eq!(tail_scroll(5, 3), 2);
        // wrap of a 10-char line at width 4 is 3 rows; 2 short lines + that + marker
        let lines = ["aa", "bbbbbbbbbb", "cc"];
        let display: u16 = lines
            .iter()
            .map(|l| split_line(l, true, 4).len() as u16)
            .sum();
        assert_eq!(display, 1 + 3 + 1);
        assert_eq!(tail_scroll(display + 1, 3), 3); // +1 marker
    }

    #[test]
    fn follow_marker_matches_grilling_copy() {
        assert_eq!(follow_marker(true, true), "── following · wrap (w) ──");
        assert_eq!(follow_marker(false, false), "── paused · truncated (w) ──");
        assert_eq!(
            follow_marker(true, false),
            "── following · truncated (w) ──"
        );
        assert_eq!(follow_marker(false, true), "── paused · wrap (w) ──");
    }

    #[test]
    fn wrap_hint_names_the_mode_w_switches_to() {
        assert_eq!(wrap_hint(true), "unwrap");
        assert_eq!(wrap_hint(false), "wrap");
    }
}

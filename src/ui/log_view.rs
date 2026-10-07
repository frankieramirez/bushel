pub fn split_line(s: &str, wrap: bool, width: u16) -> Vec<String> {
    if !wrap || width == 0 {
        return vec![s.to_string()];
    }
    wrap_line(s, width)
}

#[cfg(test)]
pub fn display_start(lines: &[String], wrap: bool, width: u16, idx: usize) -> usize {
    lines
        .iter()
        .take(idx)
        .map(|l| rows(l, wrap, width).count())
        .sum()
}

#[cfg(test)]
pub fn raw_index(lines: &[String], wrap: bool, width: u16, row: usize) -> usize {
    if lines.is_empty() {
        return 0;
    }
    let mut acc: usize = 0;
    for (i, l) in lines.iter().enumerate() {
        let n = rows(l, wrap, width).count();
        if row < acc.saturating_add(n) {
            return i;
        }
        acc = acc.saturating_add(n);
    }
    lines.len() - 1
}

pub fn tail_scroll(total: usize, height: usize) -> usize {
    total.saturating_sub(height)
}

pub fn follow_marker(follow: bool, wrap: bool) -> String {
    let state = if follow { "following" } else { "paused" };
    let mode = if wrap { "wrap" } else { "truncated" };
    format!("── {state} · {mode} (w) ──")
}

pub fn wrap_hint(wrap: bool) -> &'static str {
    if wrap { "unwrap" } else { "wrap" }
}

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[cfg(test)]
thread_local! {
    static WRAP_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Borrowed display rows: no copy of the scrollback is needed to find a viewport.
pub fn rows(s: &str, wrap: bool, width: u16) -> impl Iterator<Item = &str> {
    #[cfg(test)]
    WRAP_CALLS.with(|calls| calls.set(calls.get() + 1));
    let mut rest = Some(s);
    std::iter::from_fn(move || {
        let s = rest.take()?;
        if !wrap || width == 0 || s.is_empty() {
            return Some(s);
        }
        let mut columns = 0;
        let mut end = 0;
        for (idx, grapheme) in s.grapheme_indices(true) {
            let w = grapheme.width();
            if columns > 0 && columns + w > width as usize {
                break;
            }
            columns += w;
            end = idx + grapheme.len();
            if columns > width as usize {
                break;
            }
        }
        if end < s.len() {
            rest = Some(&s[end..]);
        }
        Some(&s[..end])
    })
}

fn wrap_line(s: &str, width: u16) -> Vec<String> {
    rows(s, true, width).map(str::to_string).collect()
}

/// Keep the editable tail visible without cutting a UTF-8 sequence or grapheme.
pub fn input_tail(s: &str, width: usize) -> &str {
    let mut start = s.len();
    let mut columns = 0;
    for (idx, grapheme) in s.grapheme_indices(true).rev() {
        let w = grapheme.width();
        if columns + w > width {
            break;
        }
        columns += w;
        start = idx;
    }
    &s[start..]
}

pub struct Window<'a> {
    pub rows: Vec<&'a str>,
    pub raw_top: usize,
    pub at_start: bool,
}

/// Following visits only the tail lines needed by the viewport. Paused mode
/// starts directly at its raw-line anchor and materializes only visible rows.
pub fn window<'a>(
    lines: impl DoubleEndedIterator<Item = &'a String> + ExactSizeIterator,
    wrap: bool,
    width: u16,
    height: usize,
    follow: bool,
    raw: usize,
) -> Window<'a> {
    if follow {
        let mut out = std::collections::VecDeque::with_capacity(height);
        let mut raw_top = lines.len().saturating_sub(1);
        let mut at_start = true;
        for (idx, line) in lines.enumerate().rev() {
            let mut tail = std::collections::VecDeque::with_capacity(height);
            for row in rows(line, wrap, width) {
                if height == 0 {
                    break;
                }
                if tail.len() == height {
                    tail.pop_front();
                    at_start = false;
                }
                tail.push_back(row);
            }
            while let Some(row) = tail.pop_back() {
                if out.len() == height {
                    at_start = false;
                    break;
                }
                out.push_front(row);
                raw_top = idx;
            }
            if out.len() == height {
                at_start &= idx == 0 && tail.is_empty();
                break;
            }
        }
        Window {
            rows: out.into_iter().collect(),
            raw_top,
            at_start,
        }
    } else {
        let raw_top = raw.min(lines.len().saturating_sub(1));
        Window {
            rows: lines
                .skip(raw_top)
                .flat_map(|s| rows(s, wrap, width))
                .take(height)
                .collect(),
            raw_top,
            at_start: raw_top == 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_log_windows_wrap_only_visible_raw_lines_at_capacity() {
        let lines = vec!["x".repeat(700); 10_000];
        for follow in [false, true] {
            WRAP_CALLS.with(|calls| calls.set(0));
            let view = window(lines.iter(), true, 80, 26, follow, 5_000);
            let wraps = WRAP_CALLS.with(|calls| calls.get());
            assert_eq!(view.rows.len(), 26);
            assert!(wraps <= 27, "follow={follow}: wrapped {wraps} raw lines");
            assert_eq!(wraps, 3);
            assert_eq!(view.raw_top, if follow { 9_997 } else { 5_000 });
        }
    }

    // Original f6c5da1 wrapping path retained only as an independent reference.
    fn legacy_split(s: &str, wrap: bool, width: u16) -> Vec<String> {
        if !wrap || width == 0 || s.is_empty() {
            return vec![s.to_string()];
        }
        let mut out = Vec::new();
        let mut current = String::new();
        let mut col = 0u16;
        for ch in s.chars() {
            let w = if ch == '\t' {
                1
            } else if ch.is_control() {
                0
            } else {
                ratatui::text::Line::from(ch.to_string()).width() as u16
            };
            if w == 0 {
                current.push(ch);
                continue;
            }
            if col > 0 && col.saturating_add(w) > width {
                out.push(std::mem::take(&mut current));
                col = 0;
            }
            current.push(ch);
            if w > width {
                out.push(std::mem::take(&mut current));
                col = 0;
            } else {
                col = col.saturating_add(w);
            }
        }
        if !current.is_empty() || out.is_empty() {
            out.push(current);
        }
        out
    }

    #[test]
    fn windows_match_the_fully_materialized_renderer_and_raw_scroll_anchor() {
        use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};
        let rings = [
            vec![],
            vec![
                "aa".to_string(),
                "bbbbbbbbbbbb".into(),
                "界e\u{301}\u{200b}界".into(),
                String::new(),
            ],
            vec!["x".repeat(700); 4],
        ];
        for lines in rings {
            for wrap in [false, true] {
                for follow in [false, true] {
                    for width in [1, 4, 9, 80] {
                        for height in [1, 3, 7] {
                            for raw in [0, 1, 999] {
                                let mut materialized: Vec<(usize, String)> = lines
                                    .iter()
                                    .enumerate()
                                    .flat_map(|(i, s)| {
                                        legacy_split(s, wrap, width)
                                            .into_iter()
                                            .map(move |row| (i, row))
                                    })
                                    .collect();
                                let anchor = raw.min(lines.len().saturating_sub(1));
                                let start = if follow {
                                    (materialized.len() + 1).saturating_sub(height as usize)
                                } else {
                                    materialized.iter().take_while(|(i, _)| *i < anchor).count()
                                };
                                let expected_raw = materialized
                                    .get(start)
                                    .map(|(i, _)| *i)
                                    .unwrap_or(lines.len().saturating_sub(1));
                                materialized.push((lines.len().saturating_sub(1), "marker".into()));
                                let view = window(
                                    lines.iter(),
                                    wrap,
                                    width,
                                    if follow {
                                        (height as usize).saturating_sub(1)
                                    } else {
                                        height as usize
                                    },
                                    follow,
                                    raw,
                                );
                                assert_eq!(
                                    view.raw_top, expected_raw,
                                    "{wrap} {follow} {width}x{height} raw={raw}"
                                );
                                let mut legacy =
                                    Terminal::new(TestBackend::new(width, height)).unwrap();
                                legacy
                                    .draw(|f| {
                                        f.render_widget(
                                            Paragraph::new(
                                                materialized
                                                    .iter()
                                                    .skip(start)
                                                    .map(|(_, row)| {
                                                        ratatui::text::Line::raw(row.as_str())
                                                    })
                                                    .collect::<Vec<_>>(),
                                            ),
                                            f.area(),
                                        )
                                    })
                                    .unwrap();
                                let mut current =
                                    Terminal::new(TestBackend::new(width, height)).unwrap();
                                current
                                    .draw(|f| {
                                        f.render_widget(
                                            Paragraph::new(
                                                view.rows
                                                    .iter()
                                                    .copied()
                                                    .chain(std::iter::once("marker"))
                                                    .map(ratatui::text::Line::raw)
                                                    .collect::<Vec<_>>(),
                                            ),
                                            f.area(),
                                        )
                                    })
                                    .unwrap();
                                assert_eq!(
                                    current.backend().buffer(),
                                    legacy.backend().buffer(),
                                    "{wrap} {follow} {width}x{height} raw={raw}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn wrapping_keeps_emoji_and_combining_graphemes_intact() {
        assert_eq!(
            split_line("👩‍💻é界e\u{301}", true, 2),
            vec!["👩‍💻", "é", "界", "e\u{301}"]
        );
    }

    #[test]
    fn following_only_visits_the_visible_tail() {
        let lines: Vec<String> = (0..10_000).map(|i| format!("line {i}")).collect();
        let visits = std::cell::Cell::new(0);
        let view = window(
            lines.iter().inspect(|_| visits.set(visits.get() + 1)),
            true,
            80,
            3,
            true,
            0,
        );
        assert_eq!(view.rows, ["line 9997", "line 9998", "line 9999"]);
        assert_eq!(view.raw_top, 9997);
        assert_eq!(visits.get(), 3);
    }

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
        let lines = vec!["aaaa".into(), "bbbbbbbbbb".into(), "cccc".into()];
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
        assert_eq!(tail_scroll(5, 3), 2);
        let lines = ["aa", "bbbbbbbbbb", "cc"];
        let display: usize = lines.iter().map(|l| split_line(l, true, 4).len()).sum();
        assert_eq!(display, 1 + 3 + 1);
        assert_eq!(tail_scroll(display + 1, 3), 3);
    }

    #[test]
    fn wrapped_display_rows_can_exceed_u16_max() {
        let width = 80u16;
        let n = 10_000;
        let lines = vec!["x".repeat(700); n];
        let rows_per = split_line(&lines[0], true, width).len();
        assert_eq!(rows_per, 9);
        let total = n * rows_per;
        assert!(total > u16::MAX as usize);

        let last = n - 1;
        let start = display_start(&lines, true, width, last);
        assert_eq!(start, last * rows_per);
        assert_eq!(raw_index(&lines, true, width, start), last);
        assert_eq!(raw_index(&lines, true, width, start + rows_per - 1), last);
        assert_eq!(raw_index(&lines, true, width, total - 1), last);
        assert_eq!(tail_scroll(total, 24), total - 24);
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

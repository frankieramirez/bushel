//! The wide table: resource type as a mode, not a rail.
//!
//! One full-width table on top, one full-width detail below. Cross-type
//! awareness moves to the header, which is also the switcher — one count per
//! noun. The trade is real and deliberate: you cannot see images while looking
//! at containers, and `2` becomes a navigation instead of an expansion.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::engine::state::{AppState, Focus, Pane};
use crate::ui::humanize::elide;
use crate::ui::layout::LayoutPlan;
use crate::ui::rail::pending_span;
use crate::ui::rows::{
    absent, age_cell, cpu_cell, mem_of_limit, reference_spans, select_bar, state_dot, uptime_cell,
};
use crate::ui::theme::{Theme, human_size};

/// One column of the table: a heading, a width, and which side the text hugs.
#[derive(Clone, Copy)]
struct Col {
    head: &'static str,
    width: u16,
    right: bool,
    /// Terminal width below which this column is not worth its cells.
    needs: u16,
}

const fn col(head: &'static str, width: u16, needs: u16) -> Col {
    Col {
        head,
        width,
        right: false,
        needs,
    }
}

const fn rcol(head: &'static str, width: u16, needs: u16) -> Col {
    Col {
        head,
        width,
        right: true,
        needs,
    }
}

/// The columns each pane offers, widest-first in the sense that later entries
/// are the first to go when the terminal is narrow.
const CONTAINER_COLS: &[Col] = &[
    col("name", 0, 0),
    col("state", 10, 62),
    col("up", 10, 74),
    rcol("cpu", 8, 44),
    rcol("mem", 15, 52),
    col("image", 26, 100),
    col("network", 14, 128),
    col("volumes", 20, 152),
    rcol("created", 10, 172),
];
const IMAGE_COLS: &[Col] = &[
    col("reference", 0, 0),
    rcol("size", 11, 44),
    rcol("created", 12, 74),
];
const VOLUME_COLS: &[Col] = &[
    col("name", 0, 0),
    col("used by", 26, 62),
    rcol("created", 12, 90),
];
const NETWORK_COLS: &[Col] = &[
    col("name", 0, 0),
    col("mode", 10, 50),
    col("subnet", 20, 62),
    col("attached", 22, 96),
    rcol("kind", 9, 110),
];

fn columns(pane: Pane) -> &'static [Col] {
    match pane {
        Pane::Containers => CONTAINER_COLS,
        Pane::Images => IMAGE_COLS,
        Pane::Volumes => VOLUME_COLS,
        Pane::Networks => NETWORK_COLS,
    }
}

/// Resolve the column widths for a given table width.
///
/// The first column is the flexible one — everything it is not spent on goes to
/// the name, which is what a person is actually scanning.
/// Cells between one column and the next.
const GUTTER: usize = 2;

fn plan_columns(pane: Pane, width: u16) -> Vec<Col> {
    let all = columns(pane);
    let mut kept: Vec<Col> = all.iter().copied().filter(|c| width >= c.needs).collect();
    let fixed: u16 = kept.iter().skip(1).map(|c| c.width + GUTTER as u16).sum();
    // 2 for the selection bar and its space, 1 for the trailing gutter.
    let flexible = width.saturating_sub(fixed + 3);
    if let Some(first) = kept.first_mut() {
        first.width = flexible.max(8);
    }
    kept
}

/// `text` fitted to its column, gutter included.
fn cell(text: &str, c: &Col) -> String {
    let w = c.width as usize;
    let text = elide(text, w);
    let used = text.chars().count();
    let space = " ".repeat(w.saturating_sub(used));
    let gutter = " ".repeat(GUTTER);
    if c.right {
        format!("{space}{text}{gutter}")
    } else {
        format!("{text}{space}{gutter}")
    }
}

/// Pad whatever spans were just pushed for `c` out to the column edge.
fn finish_cell(spans: &mut Vec<Span<'static>>, before: usize, c: &Col) {
    let drawn: usize = spans
        .iter()
        .skip(before)
        .map(|s| s.content.chars().count())
        .sum();
    spans.push(Span::raw(
        " ".repeat((c.width as usize + GUTTER).saturating_sub(drawn)),
    ));
}

pub fn draw(frame: &mut Frame, state: &AppState, th: &Theme, plan: &LayoutPlan) {
    draw_table(frame, state, th, plan.rail, plan.floor);
}

fn draw_table(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect, floor: bool) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let cols = plan_columns(state.pane, area.width);
    let mut lines: Vec<Line> = Vec::new();

    // Heading row, then a rule under it — the rule is what a border used to be.
    let mut head = vec![Span::raw("  ")];
    for c in &cols {
        head.push(Span::styled(cell(c.head, c), Style::new().fg(th.dim())));
    }
    lines.push(Line::from(head));
    if !floor {
        let glyph = if th.ascii { "-" } else { "─" };
        lines.push(Line::from(Span::styled(
            glyph.repeat(area.width as usize),
            Style::new().fg(th.dim()),
        )));
    }

    let rows_idx = state.visible_rows_for(state.pane);
    let sel = state.selected_pos_for(state.pane);
    let room = area.height.saturating_sub(lines.len() as u16) as usize;

    if rows_idx.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("  {}", empty_hint(state)),
            Style::new().fg(th.dim()),
        )));
    } else {
        let start = sel
            .map(|s| s.saturating_sub(room.saturating_sub(1)))
            .unwrap_or(0);
        for (pos, &i) in rows_idx.iter().enumerate().skip(start).take(room) {
            lines.push(row_line(state, th, i, sel == Some(pos), &cols));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn empty_hint(state: &AppState) -> &'static str {
    if state.pane_len(state.pane) > 0 {
        return "no match";
    }
    match state.pane {
        Pane::Containers => "no containers",
        Pane::Images => "no images · [u] pull one",
        Pane::Volumes => "no volumes · [c] create one",
        Pane::Networks => "no networks",
    }
}

fn row_line(
    state: &AppState,
    th: &Theme,
    idx: usize,
    selected: bool,
    cols: &[Col],
) -> Line<'static> {
    let bar = if selected {
        Span::styled(
            select_bar(th),
            Style::new().fg(if state.focus == Focus::List {
                th.accent()
            } else {
                th.dim()
            }),
        )
    } else {
        Span::raw(" ")
    };
    let mut spans = vec![bar, Span::raw(" ")];
    let get = |head: &str| cols.iter().copied().find(|c| c.head == head);
    let dim = Style::new().fg(th.dim());
    let live = Style::new().fg(th.text());

    match state.pane {
        Pane::Containers => {
            let Some(c) = state.containers.get(idx) else {
                return Line::from(spans);
            };
            let running = c.is_running();
            let name_style = if selected {
                Style::new().fg(th.text()).bold()
            } else if running {
                live
            } else {
                dim
            };
            let num_style = if running { live } else { dim };
            if let Some(name) = get("name") {
                let before = spans.len();
                spans.push(state_dot(th, running));
                if let Some(s) = pending_span(th, c.pending.is_some()) {
                    spans.push(s);
                }
                let w = Col {
                    width: name
                        .width
                        .saturating_sub(if c.pending.is_some() { 4 } else { 2 }),
                    ..name
                };
                spans.push(Span::styled(cell(&c.id, &w), name_style));
                finish_cell(&mut spans, before, &name);
            }
            if let Some(c2) = get("state") {
                let style = if running {
                    Style::new().fg(th.accent())
                } else {
                    dim
                };
                spans.push(Span::styled(cell(&c.state, &c2), style));
            }
            if let Some(c3) = get("up") {
                spans.push(Span::styled(cell(&uptime_cell(th, c), &c3), num_style));
            }
            if let Some(c4) = get("cpu") {
                spans.push(Span::styled(cell(&cpu_cell(th, c), &c4), num_style));
            }
            if let Some(c5) = get("mem") {
                spans.push(Span::styled(cell(&mem_of_limit(th, c), &c5), num_style));
            }
            if let Some(c6) = get("image") {
                let before = spans.len();
                spans.extend(reference_spans(
                    th,
                    &c.image,
                    c6.width.saturating_sub(3) as usize,
                ));
                finish_cell(&mut spans, before, &c6);
            }
            if let Some(c7) = get("network") {
                let nets: Vec<&str> = c.networks.iter().map(|(n, _)| n.as_str()).collect();
                let text = if nets.is_empty() {
                    absent(th).to_string()
                } else {
                    nets.join(",")
                };
                spans.push(Span::styled(cell(&text, &c7), dim));
            }
            if let Some(c8) = get("volumes") {
                let text = if c.volumes.is_empty() {
                    absent(th).to_string()
                } else {
                    c.volumes.join(", ")
                };
                spans.push(Span::styled(cell(&text, &c8), dim));
            }
            if let Some(c9) = get("created") {
                spans.push(Span::styled(
                    cell(&age_cell(th, c.created.as_deref()), &c9),
                    dim,
                ));
            }
        }
        Pane::Images => {
            let Some(im) = state.images.get(idx) else {
                return Line::from(spans);
            };
            if let Some(name) = get("reference") {
                let before = spans.len();
                if let Some(s) = pending_span(th, im.pending.is_some()) {
                    spans.push(s);
                }
                let w = name
                    .width
                    .saturating_sub(if im.pending.is_some() { 5 } else { 3 });
                spans.extend(reference_spans(th, &im.reference, w as usize));
                finish_cell(&mut spans, before, &name);
            }
            if let Some(size) = get("size") {
                let text = im
                    .size
                    .map(human_size)
                    .unwrap_or_else(|| absent(th).to_string());
                spans.push(Span::styled(cell(&text, &size), Style::new().fg(th.text())));
            }
            if let Some(created) = get("created") {
                spans.push(Span::styled(
                    cell(&age_cell(th, im.created.as_deref()), &created),
                    dim,
                ));
            }
        }
        Pane::Volumes => {
            let Some(v) = state.volumes.get(idx) else {
                return Line::from(spans);
            };
            if let Some(name) = get("name") {
                if let Some(s) = pending_span(th, v.pending.is_some()) {
                    spans.push(s);
                }
                let w = Col {
                    width: name
                        .width
                        .saturating_sub(if v.pending.is_some() { 2 } else { 0 }),
                    ..name
                };
                spans.push(Span::styled(cell(&v.name, &w), live));
            }
            if let Some(used) = get("used by") {
                let (text, style) = if v.in_use() {
                    (v.in_use_by.join(", "), Style::new().fg(th.yellow()))
                } else {
                    (absent(th).to_string(), dim)
                };
                spans.push(Span::styled(cell(&text, &used), style));
            }
            if let Some(created) = get("created") {
                spans.push(Span::styled(
                    cell(&age_cell(th, v.created.as_deref()), &created),
                    dim,
                ));
            }
        }
        Pane::Networks => {
            let Some(n) = state.networks.get(idx) else {
                return Line::from(spans);
            };
            if let Some(name) = get("name") {
                spans.push(Span::styled(cell(&n.name, &name), live));
            }
            if let Some(mode) = get("mode") {
                spans.push(Span::styled(
                    cell(&n.mode, &mode),
                    Style::new().fg(th.text()),
                ));
            }
            if let Some(subnet) = get("subnet") {
                let text = n.ipv4_subnet.clone().unwrap_or_else(|| absent(th).into());
                spans.push(Span::styled(cell(&text, &subnet), dim));
            }
            if let Some(attached) = get("attached") {
                let text = if n.attached.is_empty() {
                    absent(th).to_string()
                } else {
                    n.attached
                        .iter()
                        .map(|(id, _)| id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                spans.push(Span::styled(cell(&text, &attached), dim));
            }
            if let Some(badge) = get("kind") {
                let (text, style) = if n.builtin {
                    ("builtin", Style::new().fg(th.yellow()))
                } else {
                    ("", dim)
                };
                spans.push(Span::styled(cell(text, &badge), style));
            }
        }
    }
    Line::from(spans)
}

/// The header strip, which in this layout is also the pane switcher.
pub fn header_line(state: &AppState, th: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, pane) in Pane::all().into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ·   ", Style::new().fg(th.dim())));
        }
        let active = state.pane == pane;
        spans.push(Span::styled(
            format!("[{}]", pane.key()),
            Style::new().fg(th.dim()),
        ));
        spans.push(Span::styled(
            format!(" {} ", pane.title()),
            if active {
                Style::new().fg(th.accent()).bold()
            } else {
                Style::new().fg(th.dim())
            },
        ));
        spans.push(Span::styled(
            state.pane_len(pane).to_string(),
            if active {
                Style::new().fg(th.text()).bold()
            } else {
                Style::new().fg(th.dim())
            },
        ));
    }
    spans
}

/// The zoomed active panel: the same table, given the whole body.
pub fn draw_zoomed(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect) {
    draw_table(frame, state, th, area, false);
}

/// Split a row of columns for callers that need the same geometry (tests).
#[cfg(test)]
pub(crate) fn column_heads(pane: Pane, width: u16) -> Vec<&'static str> {
    plan_columns(pane, width).iter().map(|c| c.head).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_terminals_drop_columns_from_the_right() {
        assert_eq!(
            column_heads(Pane::Containers, 40),
            vec!["name"],
            "at 40 columns only the name earns its cells"
        );
        assert_eq!(
            column_heads(Pane::Containers, 80),
            vec!["name", "state", "up", "cpu", "mem"]
        );
        assert_eq!(
            column_heads(Pane::Containers, 120),
            vec!["name", "state", "up", "cpu", "mem", "image"]
        );
        assert_eq!(
            column_heads(Pane::Containers, 200),
            vec![
                "name", "state", "up", "cpu", "mem", "image", "network", "volumes", "created"
            ],
            "spare width buys columns, not padding"
        );
    }

    #[test]
    fn the_name_column_takes_whatever_the_others_leave() {
        let narrow = plan_columns(Pane::Containers, 120);
        let wide = plan_columns(Pane::Containers, 200);
        assert!(
            wide[0].width >= narrow[0].width,
            "a wider terminal never shrinks the name: {} vs {}",
            wide[0].width,
            narrow[0].width
        );
        let used: u16 = wide.iter().map(|c| c.width).sum();
        assert!(used <= 200, "columns overflow the table: {used}");
    }

    #[test]
    fn a_cell_never_exceeds_its_column() {
        let c = col("x", 6, 0);
        assert_eq!(cell("ab", &c), "ab    ".to_string() + &" ".repeat(GUTTER));
        assert_eq!(cell("abcdefghij", &c).chars().count(), 6 + GUTTER);
        let r = rcol("x", 6, 0);
        assert_eq!(cell("ab", &r), "    ab".to_string() + &" ".repeat(GUTTER));
    }
}

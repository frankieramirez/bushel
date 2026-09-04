use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::engine::state::{AppState, Focus, Pane};
use crate::ui::humanize::elide;
use crate::ui::layout::LayoutPlan;
use crate::ui::rows::{
    absent, cpu_cell, mem_cell, reference_spans, scroll_start, select_bar, state_dot,
};
use crate::ui::theme::{Theme, human_size};

const NUMBERS_MIN: usize = 30;
/// The rail caps at 36 columns, so only a zoomed section reaches this.
const IMAGE_MIN: usize = 64;
const BADGE_MIN: usize = 24;
const SUBNET_MIN: usize = 28;
const SIZE_MIN: usize = 26;
const MODE_MIN: usize = 50;
const BUILTIN_MIN: usize = 62;

fn pad(text: &str, w: usize) -> String {
    let text = elide(text, w);
    let used = text.chars().count();
    format!("{text}{}", " ".repeat(w.saturating_sub(used)))
}

fn rpad(text: &str, w: usize) -> String {
    let text = elide(text, w);
    let used = text.chars().count();
    format!("{}{text}", " ".repeat(w.saturating_sub(used)))
}

pub fn draw(frame: &mut Frame, state: &AppState, th: &Theme, plan: &LayoutPlan) {
    for (i, pane) in Pane::all().into_iter().enumerate() {
        draw_section(frame, state, th, pane, plan.slots[i], plan.tight);
    }
    draw_footer(frame, state, th, plan.footer);
    draw_divider(frame, state, th, plan);
}

fn draw_divider(frame: &mut Frame, state: &AppState, th: &Theme, plan: &LayoutPlan) {
    let area = plan.divider;
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::new().fg(if state.focus == Focus::List {
        th.accent()
    } else {
        th.dim()
    });
    if plan.stacked {
        let glyph = if th.ascii { "-" } else { "─" };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                glyph.repeat(area.width as usize),
                style,
            ))),
            area,
        );
    } else {
        let glyph = if th.ascii { "|" } else { "│" };
        for y in area.y..area.bottom() {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(glyph, style))),
                Rect {
                    y,
                    height: 1,
                    ..area
                },
            );
        }
    }
}

fn draw_footer(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect) {
    if area.height == 0 || area.width < 8 {
        return;
    }
    let reclaimable = state.reclaimable_bytes();
    let line = if reclaimable == 0 {
        Line::from(Span::styled(
            " nothing to reclaim",
            Style::new().fg(th.dim()),
        ))
    } else {
        Line::from(vec![
            Span::styled(
                format!(" {} reclaimable · ", human_size(reclaimable)),
                Style::new().fg(th.dim()),
            ),
            Span::styled("[P]", Style::new().fg(th.accent())),
            Span::styled(" prune", Style::new().fg(th.dim())),
        ])
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn label_line(state: &AppState, th: &Theme, pane: Pane, width: usize) -> Line<'static> {
    let active = pane == state.pane;
    let n = state.pane_len(pane);
    let title_style = if active {
        Style::new().fg(th.accent()).bold()
    } else {
        Style::new().fg(th.dim()).bold()
    };
    let count_style = if active {
        Style::new().fg(th.text()).bold()
    } else {
        Style::new().fg(th.dim())
    };

    let mut left: Vec<Span> = vec![
        Span::raw(" "),
        Span::styled(pane.title().to_string(), title_style),
    ];
    let mut used = 1 + pane.title().chars().count();
    if active && (state.filter_input || !state.filter.is_empty()) {
        let cursor = if state.filter_input { "▏" } else { "" };
        let text = format!("  /{}{cursor}", state.filter);
        used += text.chars().count();
        left.push(Span::styled(text, Style::new().fg(th.accent())));
    }
    let count = format!("{n} ");
    let gap = width.saturating_sub(used + count.chars().count());
    left.push(Span::raw(" ".repeat(gap)));
    left.push(Span::styled(count, count_style));
    Line::from(left)
}

fn empty_hint(state: &AppState, pane: Pane) -> &'static str {
    if state.pane_len(pane) > 0 {
        return "no match";
    }
    match pane {
        Pane::Containers => "none",
        Pane::Images => "none · [u] pull",
        Pane::Volumes => "none · [c] create",
        Pane::Networks => "none",
    }
}

fn draw_section(
    frame: &mut Frame,
    state: &AppState,
    th: &Theme,
    pane: Pane,
    area: Rect,
    tight: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let active = pane == state.pane;
    let width = area.width as usize;

    if tight && !active {
        let n = state.pane_len(pane);
        let line = Line::from(vec![
            Span::styled(format!(" {} ", pane.key()), Style::new().fg(th.dim())),
            Span::styled(pane.title().to_string(), Style::new().fg(th.dim())),
            Span::styled(format!(" {n}"), Style::new().fg(th.dim())),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let mut lines = vec![label_line(state, th, pane, width)];
    let rows_idx = state.visible_rows_for(pane);
    let sel = active.then(|| state.selected_pos_for(pane)).flatten();
    let room = area.height.saturating_sub(1) as usize;

    if rows_idx.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" {}", empty_hint(state, pane)),
            Style::new().fg(th.dim()),
        )));
    } else {
        let start = scroll_start(sel, room);
        for (pos, &i) in rows_idx.iter().enumerate().skip(start).take(room) {
            let selected = sel == Some(pos);
            lines.push(row_line(state, th, pane, i, selected, width));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn bar_span(state: &AppState, th: &Theme, selected: bool) -> Span<'static> {
    if !selected {
        return Span::raw(" ");
    }
    let color = if state.focus == Focus::List {
        th.accent()
    } else {
        th.dim()
    };
    Span::styled(select_bar(th), Style::new().fg(color))
}

fn row_line(
    state: &AppState,
    th: &Theme,
    pane: Pane,
    idx: usize,
    selected: bool,
    width: usize,
) -> Line<'static> {
    let mut spans = vec![bar_span(state, th, selected)];
    let body = width.saturating_sub(1);
    let text = |s: String, bright: bool| {
        Span::styled(
            s,
            if bright {
                Style::new().fg(th.text()).bold()
            } else {
                Style::new().fg(th.dim())
            },
        )
    };

    match pane {
        Pane::Containers => {
            let Some(c) = state.containers.get(idx) else {
                return Line::from(spans);
            };
            let num_w = if body >= NUMBERS_MIN { 6 } else { 0 };
            let mem_w = if body >= NUMBERS_MIN { 7 } else { 0 };
            let image_w = if body >= IMAGE_MIN { 28 } else { 0 };
            let name_w = body.saturating_sub(2 + num_w + mem_w + image_w);
            let num_style = Style::new().fg(if c.is_running() { th.text() } else { th.dim() });
            spans.push(state_dot(th, c.is_running()));
            if let Some(s) = pending_span(th, c.pending.is_some()) {
                spans.push(s);
            }
            let name = pad(
                &c.id,
                name_w.saturating_sub(usize::from(c.pending.is_some()) * 2),
            );
            spans.push(text(name, selected || c.is_running()));
            if num_w > 0 {
                spans.push(Span::styled(rpad(&cpu_cell(th, c), num_w), num_style));
                spans.push(Span::styled(
                    rpad(&format!("{} ", mem_cell(th, c)), mem_w),
                    num_style,
                ));
            }
            if image_w > 0 {
                spans.extend(reference_spans(th, &c.image, image_w - 4));
            }
        }
        Pane::Images => {
            let Some(im) = state.images.get(idx) else {
                return Line::from(spans);
            };
            let size_w = if body >= SIZE_MIN { 9 } else { 0 };
            let ref_w = body.saturating_sub(size_w);
            if let Some(s) = pending_span(th, im.pending.is_some()) {
                spans.push(s);
            }
            let name_w = ref_w.saturating_sub(3 + usize::from(im.pending.is_some()) * 2);
            spans.extend(reference_spans(th, &im.reference, name_w));
            if size_w > 0 {
                let used: usize = spans
                    .iter()
                    .skip(1)
                    .map(|s| s.content.chars().count())
                    .sum();
                let gap = ref_w.saturating_sub(used);
                spans.push(Span::raw(" ".repeat(gap)));
                let size = im.size.map(human_size).unwrap_or_else(|| absent(th).into());
                spans.push(Span::styled(
                    rpad(&format!("{size} "), size_w),
                    Style::new().fg(th.dim()),
                ));
            }
        }
        Pane::Volumes => {
            let Some(v) = state.volumes.get(idx) else {
                return Line::from(spans);
            };
            let badge_w = if body >= BADGE_MIN { 8 } else { 0 };
            let name_w = body.saturating_sub(badge_w + usize::from(v.pending.is_some()) * 2);
            if let Some(s) = pending_span(th, v.pending.is_some()) {
                spans.push(s);
            }
            spans.push(text(pad(&v.name, name_w), true));
            if badge_w > 0 {
                let (badge, style) = if v.in_use() {
                    ("in use ", Style::new().fg(th.yellow()))
                } else {
                    ("free ", Style::new().fg(th.dim()))
                };
                spans.push(Span::styled(rpad(badge, badge_w), style));
            }
        }
        Pane::Networks => {
            let Some(n) = state.networks.get(idx) else {
                return Line::from(spans);
            };
            let subnet = n.ipv4_subnet.clone().unwrap_or_else(|| absent(th).into());
            let sub_w = if body >= SUBNET_MIN { 17 } else { 0 };
            let mode_w = if body >= MODE_MIN { 10 } else { 0 };
            let badge_w = if body >= BUILTIN_MIN { 10 } else { 0 };
            let name_w = body.saturating_sub(sub_w + mode_w + badge_w);
            spans.push(text(pad(&n.name, name_w), true));
            if mode_w > 0 {
                spans.push(Span::styled(
                    pad(&n.mode, mode_w),
                    Style::new().fg(th.text()),
                ));
            }
            if sub_w > 0 {
                spans.push(Span::styled(
                    rpad(&format!("{subnet} "), sub_w),
                    Style::new().fg(th.dim()),
                ));
            }
            if badge_w > 0 {
                let (badge, style) = if n.builtin {
                    ("builtin ", Style::new().fg(th.yellow()))
                } else {
                    ("", Style::new().fg(th.dim()))
                };
                spans.push(Span::styled(rpad(badge, badge_w), style));
            }
        }
    }
    Line::from(spans)
}

pub(crate) fn pending_span(th: &Theme, pending: bool) -> Option<Span<'static>> {
    pending.then(|| {
        Span::styled(
            format!("{} ", th.spinner(crate::ui::draw::spinner_frame())),
            Style::new().fg(th.yellow()),
        )
    })
}

pub fn draw_zoomed(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect) {
    draw_section(frame, state, th, state.pane, area, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_never_overflows_the_column_it_was_given() {
        assert_eq!(pad("ab", 5), "ab   ");
        assert_eq!(rpad("ab", 5), "   ab");
        assert_eq!(pad("abcdefgh", 4).chars().count(), 4);
        assert_eq!(rpad("abcdefgh", 4).chars().count(), 4);
        assert!(pad("abcdefgh", 4).contains('…'), "long cells elide");
        assert_eq!(pad("abc", 0), "");
    }

    #[test]
    fn pending_rows_stay_inside_the_rail() {
        use crate::engine::state::{ActionKind, ImageEntry, Pending, PendingPhase, VolumeEntry};

        let th = Theme {
            truecolor: false,
            ascii: false,
        };
        let pending = |kind| {
            Some(Pending {
                kind,
                phase: PendingPhase::InFlight,
            })
        };
        let width = |line: &Line<'static>| -> usize {
            line.spans.iter().map(|s| s.content.chars().count()).sum()
        };

        for pending in [None, pending(ActionKind::DeleteImage)] {
            let mut state = AppState::new(true);
            state.images.push(ImageEntry {
                reference: "ghcr.io/frankieramirez/bushel:0.3.3-arm64".into(),
                size: Some(1_234_567),
                created: None,
                pending,
            });
            for w in 8..=80 {
                let line = row_line(&state, &th, Pane::Images, 0, false, w);
                assert!(
                    width(&line) <= w,
                    "image row {} wide at width {w}",
                    width(&line)
                );
            }
        }

        for pending in [None, pending(ActionKind::CreateVolume)] {
            let mut state = AppState::new(true);
            state.volumes.push(VolumeEntry {
                name: "a-very-long-volume-name-that-elides".into(),
                in_use_by: vec!["web".into()],
                created: None,
                pending,
            });
            for w in 8..=80 {
                let line = row_line(&state, &th, Pane::Volumes, 0, false, w);
                assert!(
                    width(&line) <= w,
                    "volume row {} wide at width {w}",
                    width(&line)
                );
            }
        }
    }
}

//! The persistent telemetry view at the top of a container's detail pane.
//!
//! Three shapes, chosen by the room the pane has: one row when the detail is
//! full-terminal wide (table layout), two beside the rail, three when even that
//! will not fit. The sparks are the glyph; the number beside them is the truth.

use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, RenderDirection, Sparkline};

use crate::engine::state::{AppState, ContainerEntry, TelemetrySample};
use crate::ui::rows::{absent, mem_cell, mem_of_limit};
use crate::ui::theme::Theme;

/// Below this the two-row strip has no room left for a spark worth drawing.
const TWO_ROW_MIN: u16 = 56;
/// Below this the one-row strip cannot carry cpu, mem, net and disk at once.
const ONE_ROW_MIN: u16 = 96;

const ASCII_BARS: symbols::bar::Set = symbols::bar::Set {
    full: "#",
    seven_eighths: "#",
    three_quarters: "*",
    five_eighths: "+",
    half: "=",
    three_eighths: "-",
    one_quarter: ":",
    one_eighth: ".",
    empty: " ",
};

/// How many rows the strip wants at this width.
///
/// `compact` is the table layout asking for the flattest shape that fits.
pub fn height(width: u16, compact: bool) -> u16 {
    if compact && width >= ONE_ROW_MIN {
        1
    } else if width >= TWO_ROW_MIN {
        2
    } else {
        3
    }
}

fn bar_set(ascii: bool) -> symbols::bar::Set<'static> {
    if ascii {
        ASCII_BARS
    } else {
        symbols::bar::NINE_LEVELS
    }
}

pub fn human_rate(bps: u64) -> String {
    const K: f64 = 1024.0;
    if bps < 1024 {
        format!("{bps}B/s")
    } else if (bps as f64) < K * K {
        format!("{:.1}K/s", bps as f64 / K)
    } else {
        format!("{:.1}M/s", bps as f64 / (K * K))
    }
}

fn cpu_color(th: &Theme, pct: f64) -> ratatui::style::Color {
    if pct > 90.0 {
        th.red()
    } else if pct > 70.0 {
        th.yellow()
    } else {
        th.accent()
    }
}

fn spark_vals(
    telemetry: &VecDeque<TelemetrySample>,
    f: impl Fn(&TelemetrySample) -> Option<f64>,
) -> Vec<Option<u64>> {
    telemetry
        .iter()
        .map(|s| f(s).map(|v| v.round().max(0.0) as u64))
        .collect()
}

/// One `label  value  ▁▂▃▅▇` cell of the strip.
struct Gauge<'a> {
    label: &'static str,
    value: &'a str,
    value_style: Style,
    value_w: u16,
    data: &'a [Option<u64>],
}

/// `cpu  14.1% ▁▂▃▅▇` — label, value, spark, in that order and no other.
fn gauge(frame: &mut Frame, th: &Theme, area: Rect, g: Gauge<'_>) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let parts = Layout::horizontal([
        Constraint::Length(4),
        Constraint::Length(g.value_w),
        Constraint::Min(0),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("{:<4}", g.label),
            Style::new().fg(th.dim()),
        ))),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(g.value.to_string(), g.value_style))),
        parts[1],
    );
    let spark_w = parts[2].width as usize;
    if spark_w == 0 || g.data.is_empty() {
        return;
    }
    let visible = &g.data[..g.data.len().min(spark_w)];
    frame.render_widget(
        Sparkline::default()
            .data(visible.iter().copied())
            .direction(RenderDirection::RightToLeft)
            .bar_set(bar_set(th.ascii))
            .style(Style::new().fg(th.accent())),
        parts[2],
    );
}

fn rate_line(th: &Theme, sample: Option<TelemetrySample>, running: bool) -> Vec<Span<'static>> {
    let up = if th.ascii { "^" } else { "↑" };
    let dn = if th.ascii { "v" } else { "↓" };
    let dash = absent(th);
    let rate = |v: Option<u64>| v.map(human_rate).unwrap_or_else(|| dash.into());
    let (rx, tx, r, w) = match sample {
        Some(s) if running => (rate(s.rx), rate(s.tx), rate(s.r), rate(s.w)),
        _ => (dash.into(), dash.into(), dash.into(), dash.into()),
    };
    vec![
        Span::styled("net ", Style::new().fg(th.dim())),
        Span::styled(
            format!("{up} {rx:<9} {dn} {tx:<9}"),
            Style::new().fg(th.text()),
        ),
        Span::styled("dsk ", Style::new().fg(th.dim())),
        Span::styled(format!("r {r:<9} w {w:<9}"), Style::new().fg(th.text())),
    ]
}

pub fn draw(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let selected: Option<&ContainerEntry> = state.selected_container();
    let running = selected.is_some_and(|c| c.is_running());
    let telemetry = selected.map(|c| &c.telemetry);
    let cur = telemetry.and_then(|t| t.front().copied());

    let cpu = telemetry
        .map(|t| spark_vals(t, |s| s.cpu))
        .unwrap_or_default();
    let mem = telemetry
        .map(|t| spark_vals(t, |s| s.mem))
        .unwrap_or_default();

    let dash = absent(th);
    let cpu_v = if running {
        cur.and_then(|s| s.cpu)
            .map(|v| format!("{v:.1}%"))
            .unwrap_or_else(|| dash.into())
    } else {
        dash.into()
    };
    let cpu_style = Style::new()
        .fg(if running {
            cur.and_then(|s| s.cpu)
                .map(|v| cpu_color(th, v))
                .unwrap_or_else(|| th.dim())
        } else {
            th.dim()
        })
        .bold();
    let mem_style = Style::new().fg(if running { th.text() } else { th.dim() });
    let mem_full = selected
        .map(|c| mem_of_limit(th, c))
        .unwrap_or_else(|| dash.into());
    let mem_short = selected
        .map(|c| mem_cell(th, c))
        .unwrap_or_else(|| dash.into());

    match area.height {
        1 => {
            let parts = Layout::horizontal([
                Constraint::Length(24),
                Constraint::Length(2),
                Constraint::Length(24),
                Constraint::Length(2),
                Constraint::Min(0),
            ])
            .split(area);
            gauge(
                frame,
                th,
                parts[0],
                Gauge {
                    label: "cpu",
                    value: &cpu_v,
                    value_style: cpu_style,
                    value_w: 8,
                    data: &cpu,
                },
            );
            gauge(
                frame,
                th,
                parts[2],
                Gauge {
                    label: "mem",
                    value: &mem_short,
                    value_style: mem_style,
                    value_w: 8,
                    data: &mem,
                },
            );
            frame.render_widget(
                Paragraph::new(Line::from(rate_line(th, cur, running))),
                parts[4],
            );
        }
        2 => {
            let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);
            let cols = Layout::horizontal([
                Constraint::Percentage(45),
                Constraint::Length(2),
                Constraint::Min(0),
            ])
            .split(rows[0]);
            gauge(
                frame,
                th,
                cols[0],
                Gauge {
                    label: "cpu",
                    value: &cpu_v,
                    value_style: cpu_style,
                    value_w: 8,
                    data: &cpu,
                },
            );
            gauge(
                frame,
                th,
                cols[2],
                Gauge {
                    label: "mem",
                    value: &mem_full,
                    value_style: mem_style,
                    value_w: 14,
                    data: &mem,
                },
            );
            frame.render_widget(
                Paragraph::new(Line::from(rate_line(th, cur, running))),
                rows[1],
            );
        }
        _ => {
            let rows = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(area);
            gauge(
                frame,
                th,
                rows[0],
                Gauge {
                    label: "cpu",
                    value: &cpu_v,
                    value_style: cpu_style,
                    value_w: 8,
                    data: &cpu,
                },
            );
            gauge(
                frame,
                th,
                rows[1],
                Gauge {
                    label: "mem",
                    value: &mem_short,
                    value_style: mem_style,
                    value_w: 8,
                    data: &mem,
                },
            );
            frame.render_widget(
                Paragraph::new(Line::from(rate_line(th, cur, running))),
                rows[2],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strip_flattens_only_when_the_width_can_carry_it() {
        assert_eq!(height(200, true), 1);
        assert_eq!(height(120, true), 1);
        assert_eq!(height(84, true), 2, "too narrow for one row, even compact");
        assert_eq!(height(200, false), 2, "the rail never flattens to one row");
        assert_eq!(height(40, false), 3);
        assert_eq!(height(40, true), 3);
    }

    #[test]
    fn rates_stay_instantaneous_and_short() {
        assert_eq!(human_rate(0), "0B/s");
        assert_eq!(human_rate(12_288), "12.0K/s");
        assert_eq!(human_rate(3 * 1024 * 1024), "3.0M/s");
    }
}

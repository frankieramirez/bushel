//! Rendering: consumes `AppState`, paints the frame. Never touches the Client.

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, Paragraph, Row, Table, TableState, Tabs, Wrap,
};

use crate::engine::state::{AppState, DetailTab, Focus, Overlay, Pane, Screen};
use crate::ui::layout::{LayoutFacts, LayoutPlan};
use crate::ui::log_view;
use crate::ui::theme::{ACCENT_A, ACCENT_B, Theme, human_size};

/// Values draw computes that the rest of the UI needs (effect areas, the
/// raw log line at the top of the viewport for follow-aware key handling).
#[derive(Debug, Default, Clone, Copy)]
pub struct DrawInfo {
    pub body: Rect,
    pub header: Rect,
    pub bottom: Rect,
    pub log_scroll: u16,
}

pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

fn spinner_frame() -> usize {
    // wall-clock driven so it animates whenever frames are being drawn
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    (START.get_or_init(Instant::now).elapsed().as_millis() / 100) as usize
}

pub fn draw(frame: &mut Frame, state: &AppState, th: &Theme) -> DrawInfo {
    let mut info = DrawInfo::default();
    let area = frame.area();
    frame.render_widget(
        Block::new().style(Style::new().bg(th.bg()).fg(th.text())),
        area,
    );
    match state.screen {
        Screen::Splash => draw_splash(frame, state, th),
        Screen::ServiceDown => draw_service_down(frame, state, th),
        Screen::Main => draw_main(frame, state, th, &mut info),
    }
    info
}

/// The splash never adds latency (spec): it dissolves the instant data arrives.
/// On fast startups that would mean a jarring sub-100ms flash of the mark, so it
/// only becomes visible once the probes have been running for a grace period —
/// fast start → straight into the layout, slow start → proper splash.
const SPLASH_GRACE: std::time::Duration = std::time::Duration::from_millis(150);

fn draw_splash(frame: &mut Frame, state: &AppState, th: &Theme) {
    if !state.first_run && state.started_at.elapsed() < SPLASH_GRACE {
        return; // just the ground color — the layout takes over if data beats us
    }
    let art = [
        r"   ,--./,-.                                     ",
        r"  / #   ,--\        _               _          _ ",
        r" |     |   |       | |__  _   _ ___| |__   ___| |",
        r" |     `---|       | '_ \| | | / __| '_ \ / _ \ |",
        r"  \        /       | |_) | |_| \__ \ | | |  __/ |",
        r"   `._,._,'        |_.__/ \__,_|___/_| |_|\___|_|",
    ];
    let sp = spinner_frame();
    let probes = [
        (
            "probing container system status …",
            state.cli_version.is_some(),
        ),
        ("listing containers …", state.first_data),
    ];
    let area = centered(frame.area(), 52, (art.len() + probes.len() + 3) as u16);
    let mut lines: Vec<Line> = art
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let t = i as f32 / (art.len() - 1) as f32;
            Line::from(Span::styled(
                *l,
                Style::new().fg(th.lerp(ACCENT_A, ACCENT_B, t)),
            ))
        })
        .collect();
    lines.push(Line::raw(""));
    for (p, done) in probes {
        let mark = if done { "✓" } else { th.spinner(sp) };
        let style = if done {
            Style::new().fg(th.accent())
        } else {
            Style::new().fg(th.dim())
        };
        lines.push(Line::from(Span::styled(format!("  {mark} {p}"), style)));
    }
    lines.push(Line::from(Span::styled(
        "  any key skips",
        Style::new().fg(th.dim()).italic(),
    )));
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_service_down(frame: &mut Frame, state: &AppState, th: &Theme) {
    let h = (12 + state.service_output.len() as u16).min(frame.area().height);
    let area = centered(frame.area(), 68, h);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.red()))
        .title(Line::from(th.gradient_spans(" bushel ", true)));
    let mut lines = vec![
        Line::raw(""),
        Line::from(Span::styled(
            "  the container system service is not running",
            Style::new().fg(th.red()).bold(),
        )),
        Line::raw(""),
    ];
    if state.service_starting {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", th.spinner(spinner_frame())),
                Style::new().fg(th.yellow()),
            ),
            Span::raw("starting the service … (kernel install can take a while on first run)"),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  [s]", Style::new().fg(th.accent()).bold()),
            Span::raw(" run "),
            Span::styled(
                "container system start --enable-kernel-install",
                Style::new().fg(th.yellow()),
            ),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("  [q]", Style::new().fg(th.accent()).bold()),
        Span::raw(" quit"),
    ]));
    lines.push(Line::raw(""));
    for l in &state.service_output {
        lines.push(Line::from(Span::styled(
            format!("  {l}"),
            Style::new().fg(th.dim()),
        )));
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
    if state.overlay == Overlay::MessageLog {
        draw_message_log(frame, state, th);
    }
}

fn draw_main(frame: &mut Frame, state: &AppState, th: &Theme, info: &mut DrawInfo) {
    let mut banners: Vec<Line> = Vec::new();
    if let Some(b) = &state.version_banner {
        banners.push(Line::from(vec![
            Span::styled(format!(" ⚠ {b} "), Style::new().fg(th.bg()).bg(th.yellow())),
            Span::styled("  [b] dismiss", Style::new().fg(th.yellow())),
        ]));
    }
    if state.degraded {
        banners.push(Line::from(Span::styled(
            " ⚠ polls failing to parse — showing last good state (see message log [m]) ",
            Style::new().fg(th.bg()).bg(th.red()),
        )));
    }

    let mut facts = LayoutFacts::from_state(state);
    facts.banner_rows = banners.len() as u16;
    let plan = LayoutPlan::compute(frame.area(), facts);
    info.header = plan.header;
    info.body = plan.body;
    info.bottom = plan.bottom;

    draw_header(frame, state, th, plan.header);
    for (i, b) in banners.into_iter().enumerate() {
        let area = Rect {
            y: plan.banners.y + i as u16,
            height: 1,
            ..plan.banners
        };
        frame.render_widget(Paragraph::new(b), area);
    }

    if plan.zoom {
        match state.focus {
            Focus::List => {
                draw_list_pane(frame, state, th, state.pane, plan.body, false, plan.floor)
            }
            Focus::Detail => draw_detail(frame, state, th, plan.body, info, plan.floor),
        }
    } else {
        for (i, pane) in Pane::all().into_iter().enumerate() {
            let tight = pane != state.pane && plan.tight;
            draw_list_pane(frame, state, th, pane, plan.slots[i], tight, plan.floor);
        }
        draw_detail(frame, state, th, plan.detail, info, plan.floor);
    }

    draw_bottom_bar(frame, state, th, plan.bottom, plan.floor);

    match &state.overlay {
        Overlay::ActionMenu => draw_action_menu(frame, state, th, plan.body, plan.bottom),
        Overlay::Confirm { command, .. } => draw_confirm(frame, th, command),
        Overlay::Help => draw_help(frame, th),
        Overlay::MessageLog => draw_message_log(frame, state, th),
        Overlay::PullInput { text } => draw_pull_input(frame, th, text),
        Overlay::None => {}
    }
}

fn draw_header(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect) {
    let mut spans = th.gradient_spans(" bushel ", true);
    spans.push(Span::raw("  "));
    for (i, pane) in Pane::all().into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", Style::new().fg(th.dim())));
        }
        let style = if state.pane == pane {
            Style::new().fg(th.accent()).bold().underlined()
        } else {
            Style::new().fg(th.dim())
        };
        spans.push(Span::styled(
            format!("{} {}", pane.key(), pane.title()),
            style,
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn pane_block(
    th: &Theme,
    title: &str,
    focused: bool,
    extra: Option<Line<'static>>,
) -> Block<'static> {
    let border = if focused {
        Style::new().fg(th.accent())
    } else {
        Style::new().fg(th.dim())
    };
    let mut b = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(
            format!(" {title} "),
            if focused {
                Style::new().fg(th.accent()).bold()
            } else {
                Style::new().fg(th.text())
            },
        ))
        .style(Style::new().bg(th.panel()));
    if let Some(l) = extra {
        b = b.title_bottom(l);
    }
    b
}

fn pending_span(th: &Theme, pending: bool) -> Option<Span<'static>> {
    pending.then(|| {
        Span::styled(
            format!("{} ", th.spinner(spinner_frame())),
            Style::new().fg(th.yellow()),
        )
    })
}

fn draw_list_pane(
    frame: &mut Frame,
    state: &AppState,
    th: &Theme,
    pane: Pane,
    area: Rect,
    tight: bool,
    floor: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let active = pane == state.pane;
    let focused = active && state.focus == Focus::List;
    let n = state.pane_len(pane);

    if tight {
        let style_key = if active {
            Style::new().fg(th.accent()).bold()
        } else {
            Style::new().fg(th.dim())
        };
        let style_rest = if active {
            Style::new().fg(th.text())
        } else {
            Style::new().fg(th.dim())
        };
        let line = Line::from(vec![
            Span::styled(format!(" {} ", pane.key()), style_key),
            Span::styled(pane.title().to_string(), style_rest),
            Span::styled(format!(" {n}"), Style::new().fg(th.text())),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(Style::new().bg(th.panel())),
            area,
        );
        return;
    }

    let filter_line = if active && (state.filter_input || !state.filter.is_empty()) {
        let cursor = if state.filter_input { "▏" } else { "" };
        Some(Line::from(vec![
            Span::styled(" /", Style::new().fg(th.accent()).bold()),
            Span::styled(
                format!("{}{cursor} ", state.filter),
                Style::new().fg(th.text()),
            ),
        ]))
    } else {
        None
    };
    let title = format!("{} {n}", pane.title());
    let block = pane_block(th, &title, focused, filter_line);
    let inner_h = area.height.saturating_sub(2);
    let show_header = !floor && inner_h >= 6 && active;

    let rows_idx = state.visible_rows_for(pane);
    let sel = state.selected_pos_for(pane).unwrap_or(0);
    let highlight = Style::new().bg(th.highlight()).fg(th.text()).bold();

    let (header, rows, widths): (Row, Vec<Row>, Vec<Constraint>) = match pane {
        Pane::Containers => {
            let wide = area.width >= 50 && active;
            let mid = area.width >= 32 && active;
            let rows = rows_idx
                .iter()
                .map(|&i| {
                    let c = &state.containers[i];
                    let dot = if c.is_running() {
                        Span::styled(th.dot_running(), Style::new().fg(th.accent()))
                    } else {
                        Span::styled(th.dot_stopped(), Style::new().fg(th.dim()))
                    };
                    let mut name_spans = vec![dot];
                    if let Some(s) = pending_span(th, c.pending.is_some()) {
                        name_spans.push(s);
                    }
                    name_spans.push(Span::raw(c.id.clone()));
                    let style = if c.is_running() {
                        Style::new().fg(th.text())
                    } else {
                        Style::new().fg(th.dim())
                    };
                    let cpu = c
                        .cpu_percent
                        .map(|v| format!("{v:>4.1}%"))
                        .unwrap_or_else(|| "-".into());
                    let mem = c
                        .mem_bytes
                        .map(|v| format!("{:>5.0}M", v as f64 / 1_000_000.0))
                        .unwrap_or_else(|| "-".into());
                    if wide {
                        Row::new(vec![
                            Cell::from(Line::from(name_spans)),
                            Cell::from(cpu),
                            Cell::from(mem),
                            Cell::from(c.image.clone()),
                        ])
                        .style(style)
                    } else if mid {
                        Row::new(vec![
                            Cell::from(Line::from(name_spans)),
                            Cell::from(cpu),
                            Cell::from(mem),
                        ])
                        .style(style)
                    } else {
                        Row::new(vec![Cell::from(Line::from(name_spans))]).style(style)
                    }
                })
                .collect();
            if wide {
                (
                    Row::new(vec!["name", "cpu", "mem", "image"])
                        .style(Style::new().fg(th.dim()).bold()),
                    rows,
                    vec![
                        Constraint::Min(14),
                        Constraint::Length(6),
                        Constraint::Length(7),
                        Constraint::Min(10),
                    ],
                )
            } else if mid {
                (
                    Row::new(vec!["name", "cpu", "mem"]).style(Style::new().fg(th.dim()).bold()),
                    rows,
                    vec![
                        Constraint::Min(14),
                        Constraint::Length(6),
                        Constraint::Length(7),
                    ],
                )
            } else {
                (
                    Row::new(vec!["name"]).style(Style::new().fg(th.dim()).bold()),
                    rows,
                    vec![Constraint::Min(10)],
                )
            }
        }
        Pane::Images => {
            let with_size = active && area.width >= 40;
            let rows = rows_idx
                .iter()
                .map(|&i| {
                    let im = &state.images[i];
                    let mut spans = Vec::new();
                    if let Some(s) = pending_span(th, im.pending.is_some()) {
                        spans.push(s);
                    }
                    spans.push(Span::raw(im.reference.clone()));
                    if with_size {
                        let size = im.size.map(human_size).unwrap_or_else(|| "-".into());
                        Row::new(vec![Cell::from(Line::from(spans)), Cell::from(size)])
                    } else {
                        Row::new(vec![Cell::from(Line::from(spans))])
                    }
                })
                .collect();
            if with_size {
                (
                    Row::new(vec!["reference", "size"]).style(Style::new().fg(th.dim()).bold()),
                    rows,
                    vec![Constraint::Min(16), Constraint::Length(9)],
                )
            } else {
                (
                    Row::new(vec!["reference"]).style(Style::new().fg(th.dim()).bold()),
                    rows,
                    vec![Constraint::Min(10)],
                )
            }
        }
        Pane::Volumes => {
            let rows = rows_idx
                .iter()
                .map(|&i| {
                    let v = &state.volumes[i];
                    let mut spans = Vec::new();
                    if let Some(s) = pending_span(th, v.pending.is_some()) {
                        spans.push(s);
                    }
                    spans.push(Span::raw(v.name.clone()));
                    let badge = if v.in_use() {
                        Span::styled("in use", Style::new().fg(th.yellow()))
                    } else {
                        Span::styled("-", Style::new().fg(th.dim()))
                    };
                    Row::new(vec![
                        Cell::from(Line::from(spans)),
                        Cell::from(Line::from(badge)),
                    ])
                })
                .collect();
            (
                Row::new(vec!["name", ""]).style(Style::new().fg(th.dim()).bold()),
                rows,
                vec![Constraint::Min(10), Constraint::Length(8)],
            )
        }
    };

    let mut table = Table::new(rows, widths)
        .block(block)
        .row_highlight_style(highlight)
        .highlight_symbol("");
    if show_header {
        table = table.header(header);
    }
    let mut ts = TableState::default();
    ts.select((!rows_idx.is_empty()).then_some(sel));
    frame.render_stateful_widget(table, area, &mut ts);
}

fn draw_detail(
    frame: &mut Frame,
    state: &AppState,
    th: &Theme,
    area: Rect,
    info: &mut DrawInfo,
    floor: bool,
) {
    if area.height == 0 {
        return;
    }
    let focused = state.focus == Focus::Detail;
    let block = pane_block(th, "detail", focused, None);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // an active pull streams raw CLI progress in the detail pane — never a modal
    if let Some(pull) = &state.pull {
        if state.pane == Pane::Images {
            let mut lines = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled(
                        format!("  {} ", th.spinner(spinner_frame())),
                        Style::new().fg(th.yellow()),
                    ),
                    Span::styled(
                        format!(
                            "pulling {}  ({}s)",
                            pull.reference,
                            pull.started.elapsed().as_secs()
                        ),
                        Style::new().fg(th.text()).bold(),
                    ),
                ]),
                Line::raw(""),
            ];
            let h = inner.height.saturating_sub(3) as usize;
            let start = pull.lines.len().saturating_sub(h);
            for l in &pull.lines[start..] {
                lines.push(Line::from(Span::styled(
                    format!("  {l}"),
                    Style::new().fg(th.dim()),
                )));
            }
            frame.render_widget(Paragraph::new(lines), inner);
            return;
        }
    }

    let mut content_area = inner;
    let show_tabs = !floor && state.pane == Pane::Containers && inner.height >= 6;
    if show_tabs {
        let tabs_area = Rect { height: 1, ..inner };
        content_area = Rect {
            y: inner.y + 1,
            height: inner.height.saturating_sub(1),
            ..inner
        };
        let idx = if state.detail_tab == DetailTab::Logs {
            0
        } else {
            1
        };
        let tabs = Tabs::new(vec![" Logs [l] ", " Inspect [i] "])
            .select(idx)
            .style(Style::new().fg(th.dim()))
            .highlight_style(Style::new().fg(th.accent()).bold().underlined());
        frame.render_widget(tabs, tabs_area);
    }

    let inspect_lines = |id: Option<&str>| -> Vec<Line> {
        let Some(id) = id else {
            return vec![Line::raw("no selection")];
        };
        match state.inspect_cache.get(id) {
            Some(json) => json
                .lines()
                .map(|l| {
                    let style = if l.trim_start().starts_with('"') {
                        Style::new().fg(th.text())
                    } else {
                        Style::new().fg(th.dim())
                    };
                    Line::from(Span::styled(l.to_string(), style))
                })
                .collect(),
            None => vec![Line::from(Span::styled(
                format!("{} loading inspect …", th.spinner(spinner_frame())),
                Style::new().fg(th.dim()),
            ))],
        }
    };

    let (lines, follow_tail): (Vec<Line>, bool) = match state.pane {
        Pane::Containers => match state.detail_tab {
            DetailTab::Logs => {
                let width = content_area.width;
                let mut l: Vec<Line> = Vec::new();
                if state.logs_loading {
                    l.push(Line::from(Span::styled(
                        format!("{} loading log backlog …", th.spinner(spinner_frame())),
                        Style::new().fg(th.dim()),
                    )));
                }
                let log_style = Style::new().fg(th.text());
                for s in &state.log_lines {
                    for row in log_view::split_line(s, state.wrap, width) {
                        l.push(Line::from(Span::styled(row, log_style)));
                    }
                }
                let marker = if state.selected_container().is_none() {
                    Span::styled("── no selection ──", Style::new().fg(th.dim()))
                } else if state.log_owner.is_none() {
                    Span::styled(
                        "── container not running: no live logs ──",
                        Style::new().fg(th.dim()),
                    )
                } else if state.follow_ended {
                    Span::styled("── follow ended ──", Style::new().fg(th.dim()))
                } else {
                    let style = if state.follow {
                        Style::new().fg(th.accent())
                    } else {
                        Style::new().fg(th.dim())
                    };
                    Span::styled(log_view::follow_marker(state.follow, state.wrap), style)
                };
                l.push(Line::from(marker));
                (l, state.follow)
            }
            DetailTab::Inspect => (
                inspect_lines(state.selected_container().map(|c| c.id.as_str())),
                false,
            ),
        },
        Pane::Images => (
            inspect_lines(state.selected_image().map(|i| i.reference.as_str())),
            false,
        ),
        Pane::Volumes => (
            inspect_lines(state.selected_volume().map(|v| v.name.as_str())),
            false,
        ),
    };

    let logs = state.pane == Pane::Containers && state.detail_tab == DetailTab::Logs;
    if logs {
        // Wrap can push display rows past u16::MAX; Paragraph::scroll is (u16, u16),
        // so rebase onto a pane-height window and render that at scroll 0.
        let total = lines.len();
        let h = content_area.height as usize;
        let width = content_area.width;
        let prefix = if state.logs_loading { 1 } else { 0 };
        let scroll = if follow_tail {
            log_view::tail_scroll(total, h)
        } else {
            let raw = (state.detail_scroll as usize).min(state.log_lines.len().saturating_sub(1));
            log_view::display_start(&state.log_lines, state.wrap, width, raw)
                .saturating_add(prefix)
                .min(total.saturating_sub(1))
        };
        info.log_scroll = log_view::raw_index(
            &state.log_lines,
            state.wrap,
            width,
            scroll.saturating_sub(prefix),
        ) as u16;
        let end = scroll.saturating_add(h).min(total);
        let window = lines.get(scroll..end).unwrap_or(&[]).to_vec();
        frame.render_widget(Paragraph::new(window), content_area);
    } else {
        let total = lines.len() as u16;
        let scroll = state.detail_scroll.min(total.saturating_sub(1));
        frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), content_area);
    }
}

fn draw_bottom_bar(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect, floor: bool) {
    let hint_style = Style::new().fg(th.dim());
    let key_style = Style::new().fg(th.accent());
    let mut spans: Vec<Span> = Vec::new();
    if let Some(t) = &state.toast {
        let style = if t.error {
            Style::new().fg(th.red()).bold()
        } else {
            Style::new().fg(th.accent())
        };
        spans.push(Span::styled(format!(" {}", t.text), style));
    } else if let Some(a) = &state.activity {
        spans.push(Span::styled(
            format!(
                " {} {}  ({}s)",
                th.spinner(spinner_frame()),
                a.label,
                a.started.elapsed().as_secs()
            ),
            Style::new().fg(th.yellow()),
        ));
    } else {
        let hints: &[(&str, &str)] = if floor {
            &[
                ("1/2/3", "expand"),
                ("j/k", "move"),
                ("/", "filter"),
                ("f", "zoom"),
            ]
        } else if state.focus == Focus::List {
            &[
                ("1/2/3", "expand"),
                ("j/k", "move"),
                ("enter", "focus"),
                ("/", "filter"),
                ("f", "zoom"),
                ("?", "help"),
            ]
        } else {
            &[
                ("j/k", "scroll"),
                ("l/i", "tabs"),
                ("F", "follow"),
                ("w", log_view::wrap_hint(state.wrap)),
                ("esc", "back"),
                ("f", "zoom"),
            ]
        };
        for (k, v) in hints {
            spans.push(Span::styled(format!(" {k}"), key_style));
            spans.push(Span::styled(format!(" {v} "), hint_style));
        }
    }

    // status cluster, right-aligned: service dot, CLI version, poll spinner.
    // Dropped at the 55×20 floor and whenever it wouldn't fit.
    if !floor {
        let service_up = state.screen != Screen::ServiceDown;
        let version = state.cli_version.clone().unwrap_or_else(|| "?".into());
        let sp = th.spinner(spinner_frame());
        let cluster_len = format!("● service  container {version}  {sp} ")
            .chars()
            .count();
        let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        if (area.width as usize) > used + cluster_len {
            let pad = (area.width as usize).saturating_sub(used + cluster_len);
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled(
                if th.ascii { "* " } else { "● " },
                Style::new().fg(if service_up { th.accent() } else { th.red() }),
            ));
            spans.push(Span::styled("service  ", hint_style));
            spans.push(Span::styled(format!("container {version}  "), hint_style));
            spans.push(Span::styled(sp.to_string(), Style::new().fg(th.dim())));
            spans.push(Span::raw(" "));
        }
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::new().bg(th.bar())),
        area,
    );
}

fn draw_action_menu(frame: &mut Frame, state: &AppState, th: &Theme, body: Rect, bottom: Rect) {
    let items = state.available_actions();
    let h = items.len() as u16 + 2;
    let area = Rect {
        x: body.x,
        y: bottom.y.saturating_sub(h),
        width: body.width,
        height: h,
    };
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Span::styled(
            " actions ",
            Style::new().fg(th.accent()).bold(),
        ))
        .style(Style::new().bg(th.panel()));
    let lines: Vec<Line> = items
        .iter()
        .map(|it| {
            let style = if it.destructive {
                Style::new().fg(th.red())
            } else {
                Style::new().fg(th.text())
            };
            Line::from(vec![
                Span::styled(
                    format!("  {}  ", it.key),
                    Style::new().fg(th.accent()).bold(),
                ),
                Span::styled(it.label.to_string(), style),
                if it.destructive {
                    Span::styled("  (confirms)", Style::new().fg(th.dim()))
                } else {
                    Span::raw("")
                },
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_confirm(frame: &mut Frame, th: &Theme, command: &str) {
    let w = (command.chars().count() as u16 + 8)
        .max(44)
        .min(frame.area().width);
    let area = centered(frame.area(), w, 7);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.red()))
        .title(Span::styled(" confirm ", Style::new().fg(th.red()).bold()))
        .style(Style::new().bg(th.panel()));
    let lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  $ "),
            Span::styled(command.to_string(), Style::new().fg(th.yellow()).bold()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  [y]", Style::new().fg(th.accent()).bold()),
            Span::raw(" run   "),
            Span::styled("[esc]", Style::new().fg(th.dim()).bold()),
            Span::raw(" cancel"),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_pull_input(frame: &mut Frame, th: &Theme, text: &str) {
    let area = centered(frame.area(), 56, 4);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Span::styled(
            " pull image ",
            Style::new().fg(th.accent()).bold(),
        ))
        .style(Style::new().bg(th.panel()));
    let lines = vec![
        Line::from(vec![
            Span::raw(" reference: "),
            Span::styled(format!("{text}▏"), Style::new().fg(th.text())),
        ]),
        Line::from(Span::styled(
            " enter pulls (tag defaults to :latest) · esc cancels",
            Style::new().fg(th.dim()),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_help(frame: &mut Frame, th: &Theme) {
    let area = centered(frame.area(), 68, 23);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Line::from(th.gradient_spans(" keys ", true)))
        .style(Style::new().bg(th.panel()));
    let g = |s: &str| {
        Line::from(Span::styled(
            s.to_string(),
            Style::new().fg(th.accent()).bold(),
        ))
    };
    let k = |key: &str, desc: &str| {
        Line::from(vec![
            Span::styled(format!("  {key:<12}"), Style::new().fg(th.yellow())),
            Span::raw(desc.to_string()),
        ])
    };
    let lines = vec![
        g(" global"),
        k("1/2/3, tab", "expand pane (containers / images / volumes)"),
        k("f", "zoom focused side"),
        k("m", "message log"),
        k("b", "dismiss version banner"),
        k("q", "quit"),
        g(" list"),
        k("j/k g/G", "move / top / bottom"),
        k("/", "fuzzy filter (esc clears)"),
        k("enter", "focus detail pane"),
        k("space", "action menu"),
        k(
            "s r K d P e",
            "start/stop · restart · kill · delete · prune · exec",
        ),
        k("u", "pull image (images pane)"),
        g(" detail"),
        k("l / i", "logs / inspect tab (containers)"),
        k("F", "toggle follow"),
        k("w", "toggle wrap / truncated"),
        k("pgup/pgdn", "scroll without switching focus"),
        k("esc", "back to list"),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_message_log(frame: &mut Frame, state: &AppState, th: &Theme) {
    let full = frame.area();
    let area = Rect {
        x: full.x + 2,
        y: full.y + full.height / 2,
        width: full.width.saturating_sub(4),
        height: (full.height / 2).saturating_sub(1),
    };
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Span::styled(
            " message log ",
            Style::new().fg(th.accent()).bold(),
        ))
        .style(Style::new().bg(th.panel()));
    let lines: Vec<Line> = state
        .messages
        .iter()
        .rev()
        .flat_map(|m| {
            m.split('\n')
                .map(|s| Line::raw(format!(" {s}")).style(Style::new().fg(th.text())))
                .collect::<Vec<_>>()
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::state::{ContainerEntry, ImageEntry, VolumeEntry};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn sample() -> AppState {
        let mut s = AppState::new(true);
        s.cli_version = Some("1.2.0".into());
        s.containers.push(ContainerEntry {
            id: "qtest".into(),
            image: "alpine:latest".into(),
            state: "running".into(),
            created: None,
            cpus: None,
            volumes: vec![],
            cpu_percent: Some(1.2),
            mem_bytes: Some(4_000_000),
            pending: None,
        });
        s.images.push(ImageEntry {
            reference: "alpine:latest".into(),
            size: Some(8_300_000),
            created: None,
            pending: None,
        });
        s.volumes.push(VolumeEntry {
            name: "qvol".into(),
            in_use_by: vec!["qtest".into()],
            created: None,
            pending: None,
        });
        s.clamp_selection();
        s
    }

    fn render(w: u16, h: u16, state: &AppState) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        let th = Theme {
            truecolor: false,
            ascii: true,
        };
        terminal
            .draw(|f| {
                draw(f, state, &th);
            })
            .unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let area = buf.area();
        let mut out = String::new();
        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                let cell = &buf[(x, y)];
                let sym = cell.symbol();
                if !sym.is_empty() {
                    line.push_str(sym);
                }
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    #[test]
    fn floor_55x20_keeps_all_three_panes_and_drops_floor_chrome() {
        let frame = render(55, 20, &sample());
        // header is a pane switcher without counts
        let header = frame.lines().next().unwrap();
        assert!(header.contains("1 containers"), "{header}");
        assert!(header.contains("2 images"), "{header}");
        assert!(header.contains("3 volumes"), "{header}");
        assert!(
            !header.contains("containers 1"),
            "counts live on the rail, not the header: {header}"
        );
        // 1-row header: the next line is the rail, not a blank
        let second = frame.lines().nth(1).unwrap();
        assert!(
            second.contains("containers"),
            "expected 1-row header then rail, got {second:?}"
        );
        // counts live on the rail; inactive panes collapse to 1-row title+count
        assert!(frame.contains("containers 1"), "{frame}");
        assert!(frame.contains("2 images 1"), "{frame}");
        assert!(frame.contains("3 volumes 1"), "{frame}");
        // no table headers, no Logs/Inspect tab row, no status cluster
        assert!(!frame.contains("cpu"), "{frame}");
        assert!(!frame.contains("Logs [l]"), "{frame}");
        assert!(!frame.contains("Inspect [i]"), "{frame}");
        assert!(!frame.contains("service"), "{frame}");
    }

    #[test]
    fn wide_200x50_keeps_the_rail_cap() {
        let frame = render(200, 50, &sample());
        assert!(frame.contains("containers 1"), "{frame}");
        assert!(frame.contains("images 1"), "{frame}");
        assert!(frame.contains("volumes 1"), "{frame}");
        assert!(frame.contains("Logs [l]"), "{frame}");
        assert!(frame.contains("service"), "{frame}");
        // spare width belongs to logs: the rail's top-right corner is at col 36
        let body = frame.lines().nth(2).unwrap();
        let rail_end = body.chars().position(|c| c == '╮').expect(body);
        assert!(
            rail_end < 36,
            "rail should cap at 36, first ╮ at {rail_end}: {body}"
        );
    }

    #[test]
    fn medium_100x30_sits_beside_with_roomy_collapse() {
        let frame = render(100, 30, &sample());
        assert!(frame.contains("containers 1"), "{frame}");
        assert!(frame.contains("images 1"), "{frame}");
        assert!(frame.contains("volumes 1"), "{frame}");
        assert!(frame.contains("Logs [l]"), "{frame}");
        assert!(frame.contains("service"), "{frame}");
        // table headers on the active panel
        assert!(frame.contains("cpu"), "{frame}");
        // 1-row header is not used: a blank line sits under the switcher
        let second = frame.lines().nth(1).unwrap();
        assert!(
            second.trim().is_empty(),
            "expected 2-row header, got {second:?}"
        );
    }

    #[test]
    fn zoom_fullscreens_the_active_panel_table_not_the_rail() {
        let mut s = sample();
        s.zoom = true;
        s.focus = Focus::List;
        let frame = render(100, 30, &s);
        assert!(frame.contains("containers 1"), "{frame}");
        assert!(!frame.contains("images 1"), "{frame}");
        assert!(!frame.contains("volumes 1"), "{frame}");
        assert!(!frame.contains("Logs [l]"), "{frame}");
    }

    #[test]
    fn zoom_detail_fullscreens_the_detail_pane() {
        let mut s = sample();
        s.zoom = true;
        s.focus = Focus::Detail;
        let frame = render(100, 30, &s);
        assert!(frame.contains("Logs [l]"), "{frame}");
        assert!(!frame.contains("containers 1"), "{frame}");
        assert!(!frame.contains("images 1"), "{frame}");
    }

    #[test]
    fn images_and_volumes_have_inspect_only() {
        let mut s = sample();
        s.pane = Pane::Images;
        let frame = render(100, 30, &s);
        assert!(frame.contains("images 1"), "{frame}");
        assert!(!frame.contains("Logs [l]"), "{frame}");
    }

    #[test]
    fn expanding_a_pane_keeps_the_others_on_the_rail() {
        let mut s = sample();
        s.pane = Pane::Images;
        let frame = render(55, 20, &s);
        assert!(frame.contains("images 1"), "{frame}");
        assert!(frame.contains("1 containers 1"), "{frame}");
        assert!(frame.contains("3 volumes 1"), "{frame}");
        assert!(!frame.contains("Logs [l]"), "{frame}");
    }
}

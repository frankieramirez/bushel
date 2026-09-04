use std::collections::VecDeque;
use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, Paragraph, RenderDirection, Row, Sparkline, Table, TableState,
    Tabs, Wrap,
};

use crate::engine::state::{
    AppState, ContainerEntry, DetailTab, Focus, Overlay, Pane, Screen, TelemetrySample,
};
use crate::ui::help::{HELP, HELP_KEY_COL};
use crate::ui::layout::{self, LayoutFacts, LayoutPlan, centered};
use crate::ui::log_view;
use crate::ui::theme::{ACCENT_A, ACCENT_B, Theme, human_size};

const STRIP_HEIGHT: u16 = 3;
const STRIP_MIN_LOG: u16 = 4;

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

#[derive(Debug, Default, Clone, Copy)]
pub struct DrawInfo {
    pub body: Rect,
    pub header: Rect,
    pub bottom: Rect,
    pub log_scroll: u16,
    pub help_max_scroll: u16,
}

fn spinner_frame() -> usize {
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

const SPLASH_GRACE: std::time::Duration = std::time::Duration::from_millis(150);

fn draw_splash(frame: &mut Frame, state: &AppState, th: &Theme) {
    if !state.first_run && state.started_at.elapsed() < SPLASH_GRACE {
        return;
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
        Overlay::ActionMenu => draw_action_menu(frame, state, th, plan.detail, plan.floor),
        Overlay::Confirm { command, .. } => draw_confirm(frame, th, command),
        Overlay::Help => info.help_max_scroll = draw_help(frame, state, th),
        Overlay::MessageLog => draw_message_log(frame, state, th),
        Overlay::PullInput { text } => draw_pull_input(frame, th, text),
        Overlay::TagInput { text } => draw_tag_input(frame, th, text),
        Overlay::CreateVolumeInput { text } => draw_create_volume_input(frame, th, text),
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
        Pane::Networks => {
            let wide = active && area.width >= 40;
            let rows = rows_idx
                .iter()
                .map(|&i| {
                    let n = &state.networks[i];
                    let badge = if n.builtin {
                        Span::styled("builtin", Style::new().fg(th.yellow()))
                    } else {
                        Span::styled("-", Style::new().fg(th.dim()))
                    };
                    if wide {
                        Row::new(vec![
                            Cell::from(n.name.clone()),
                            Cell::from(n.mode.clone()),
                            Cell::from(n.ipv4_subnet.clone().unwrap_or_else(|| "-".into())),
                            Cell::from(Line::from(badge)),
                        ])
                    } else {
                        Row::new(vec![
                            Cell::from(n.name.clone()),
                            Cell::from(Line::from(badge)),
                        ])
                    }
                })
                .collect();
            if wide {
                (
                    Row::new(vec!["name", "mode", "subnet", ""])
                        .style(Style::new().fg(th.dim()).bold()),
                    rows,
                    vec![
                        Constraint::Min(8),
                        Constraint::Length(8),
                        Constraint::Min(10),
                        Constraint::Length(8),
                    ],
                )
            } else {
                (
                    Row::new(vec!["name", ""]).style(Style::new().fg(th.dim()).bold()),
                    rows,
                    vec![Constraint::Min(8), Constraint::Length(8)],
                )
            }
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

    if state.pane == Pane::Containers && content_area.height >= STRIP_HEIGHT + STRIP_MIN_LOG {
        let parts = Layout::vertical([Constraint::Length(STRIP_HEIGHT), Constraint::Min(1)])
            .split(content_area);
        draw_strip(frame, state, th, parts[0]);
        content_area = parts[1];
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
        Pane::Networks => {
            let mut lines = Vec::new();
            if let Some(n) = state.selected_network() {
                lines.push(Line::from(Span::styled(
                    "containers on this network",
                    Style::new().fg(th.dim()),
                )));
                if n.attached.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "  none",
                        Style::new().fg(th.dim()),
                    )));
                } else {
                    for (id, addr) in &n.attached {
                        let addr = addr.as_deref().unwrap_or("-");
                        lines.push(Line::from(Span::styled(
                            format!("  {id}  {addr}"),
                            Style::new().fg(th.text()),
                        )));
                    }
                }
                lines.push(Line::raw(""));
            }
            lines.extend(inspect_lines(
                state.selected_network().map(|n| n.name.as_str()),
            ));
            (lines, false)
        }
    };

    let logs = state.pane == Pane::Containers && state.detail_tab == DetailTab::Logs;
    if logs {
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

fn bar_set(ascii: bool) -> symbols::bar::Set<'static> {
    if ascii {
        ASCII_BARS
    } else {
        symbols::bar::NINE_LEVELS
    }
}

fn human_rate(bps: u64) -> String {
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

fn draw_spark(
    frame: &mut Frame,
    th: &Theme,
    area: Rect,
    label: &str,
    value: String,
    value_style: Style,
    data: &[Option<u64>],
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let parts = Layout::horizontal([
        Constraint::Length(4),
        Constraint::Length(7),
        Constraint::Min(1),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("{label:<4}"),
            Style::new().fg(th.dim()),
        ))),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(value, value_style))),
        parts[1],
    );
    let spark_w = parts[2].width as usize;
    if spark_w == 0 || data.is_empty() {
        return;
    }
    let visible = &data[..data.len().min(spark_w)];
    frame.render_widget(
        Sparkline::default()
            .data(visible.iter().copied())
            .direction(RenderDirection::RightToLeft)
            .bar_set(bar_set(th.ascii))
            .style(Style::new().fg(th.accent())),
        parts[2],
    );
}

fn draw_rates(
    frame: &mut Frame,
    th: &Theme,
    area: Rect,
    sample: Option<TelemetrySample>,
    running: bool,
) {
    if area.width == 0 {
        return;
    }
    let up = if th.ascii { "^" } else { "↑" };
    let dn = if th.ascii { "v" } else { "↓" };
    let rate = |v: Option<u64>| v.map(human_rate).unwrap_or_else(|| "-".into());
    let (rx, tx, r, w) = match sample {
        Some(s) if running => (rate(s.rx), rate(s.tx), rate(s.r), rate(s.w)),
        _ => ("-".into(), "-".into(), "-".into(), "-".into()),
    };
    let net = format!("net {up}{rx:<7} {dn}{tx:<7}");
    let dsk = format!("dsk r {r:<7} w {w:<7}");
    let line = Line::from(vec![
        Span::styled(net, Style::new().fg(th.text())),
        Span::styled("  ", Style::new()),
        Span::styled(dsk, Style::new().fg(th.dim())),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_strip(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect) {
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

    let (cpu_v, mem_v) = if running {
        (
            cur.and_then(|s| s.cpu)
                .map(|v| format!("{v:>5.1}%"))
                .unwrap_or_else(|| "    -".into()),
            cur.and_then(|s| s.mem)
                .map(|v| format!("{v:>5.0}%"))
                .unwrap_or_else(|| "    -".into()),
        )
    } else {
        ("    -".into(), "    -".into())
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

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);
    draw_spark(frame, th, rows[0], "cpu", cpu_v, cpu_style, &cpu);
    draw_spark(frame, th, rows[1], "mem", mem_v, mem_style, &mem);
    draw_rates(frame, th, rows[2], cur, running);
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
                ("1/2/3/4", "expand"),
                ("j/k", "move"),
                ("/", "filter"),
                ("f", "zoom"),
            ]
        } else if state.focus == Focus::List {
            &[
                ("1/2/3/4", "expand"),
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

fn draw_action_menu(frame: &mut Frame, state: &AppState, th: &Theme, detail: Rect, floor: bool) {
    let items = layout::sheet_items(state.available_actions(), floor);
    let area = layout::action_sheet(detail, items.len() as u16);
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
    let area = layout::confirm_modal(frame.area());
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.red()))
        .title(Span::styled(" confirm ", Style::new().fg(th.red()).bold()))
        .style(Style::new().bg(th.panel()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let keys = Rect {
        y: inner.bottom() - 1,
        height: 1,
        ..inner
    };
    let body = Rect {
        height: inner.height - 1,
        ..inner
    };
    let mut rows = log_view::split_line(command, true, body.width.saturating_sub(4));
    let room = body.height as usize;
    let mut lines = Vec::new();
    if rows.len() < room {
        lines.push(Line::raw(""));
    } else if rows.len() > room {
        rows.truncate(room);
        if let Some(last) = rows.last_mut() {
            last.pop();
            last.push('…');
        }
    }
    for (i, row) in rows.iter().enumerate() {
        let prefix = if i == 0 { "  $ " } else { "    " };
        lines.push(Line::from(vec![
            Span::raw(prefix),
            Span::styled(row.clone(), Style::new().fg(th.yellow()).bold()),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), body);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  [y]", Style::new().fg(th.accent()).bold()),
            Span::raw(" run   "),
            Span::styled("[esc]", Style::new().fg(th.dim()).bold()),
            Span::raw(" cancel"),
        ])),
        keys,
    );
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

fn draw_tag_input(frame: &mut Frame, th: &Theme, text: &str) {
    let area = centered(frame.area(), 56, 4);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Span::styled(
            " tag image ",
            Style::new().fg(th.accent()).bold(),
        ))
        .style(Style::new().bg(th.panel()));
    let lines = vec![
        Line::from(vec![
            Span::raw(" new reference: "),
            Span::styled(format!("{text}▏"), Style::new().fg(th.text())),
        ]),
        Line::from(Span::styled(
            " enter continues to confirm · esc cancels",
            Style::new().fg(th.dim()),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_create_volume_input(frame: &mut Frame, th: &Theme, text: &str) {
    let area = centered(frame.area(), 56, 4);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Span::styled(
            " create volume ",
            Style::new().fg(th.accent()).bold(),
        ))
        .style(Style::new().bg(th.panel()));
    let lines = vec![
        Line::from(vec![
            Span::raw(" name: "),
            Span::styled(format!("{text}▏"), Style::new().fg(th.text())),
        ]),
        Line::from(Span::styled(
            " enter continues to confirm · esc cancels",
            Style::new().fg(th.dim()),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub(crate) fn help_lines(th: &Theme, width: u16) -> Vec<Line<'static>> {
    let desc_w = width.saturating_sub(HELP_KEY_COL).max(8);
    let mut out = Vec::new();
    for row in HELP {
        if row.keys.is_empty() {
            out.push(Line::from(Span::styled(
                row.desc.to_string(),
                Style::new().fg(th.accent()).bold(),
            )));
            continue;
        }
        for (i, chunk) in log_view::split_line(row.desc, true, desc_w)
            .into_iter()
            .enumerate()
        {
            let key = if i == 0 { row.keys } else { "" };
            out.push(Line::from(vec![
                Span::styled(format!("  {key:<12}"), Style::new().fg(th.yellow())),
                Span::raw(chunk),
            ]));
        }
    }
    out
}

fn draw_help(frame: &mut Frame, state: &AppState, th: &Theme) -> u16 {
    let full = frame.area();
    let lines = help_lines(th, layout::help_inner_width(full));
    let area = layout::help_modal(full, lines.len() as u16);
    frame.render_widget(Clear, area);
    let visible = area.height.saturating_sub(2);
    let max_scroll = (lines.len() as u16).saturating_sub(visible);
    let scroll = state.help_scroll.min(max_scroll);
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Line::from(th.gradient_spans(" keys ", true)))
        .style(Style::new().bg(th.panel()));
    if max_scroll > 0 {
        block = block.title_bottom(Line::from(Span::styled(
            " j/k scroll · esc close ",
            Style::new().fg(th.dim()),
        )));
    }
    frame.render_widget(Paragraph::new(lines).block(block).scroll((scroll, 0)), area);
    max_scroll
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
    use crate::engine::state::{ContainerEntry, ImageEntry, NetworkEntry, VolumeEntry};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::VecDeque;

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
            networks: vec![],
            cpu_percent: Some(1.2),
            mem_bytes: Some(4_000_000),
            telemetry: VecDeque::new(),
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
        s.networks.push(NetworkEntry {
            name: "default".into(),
            mode: "nat".into(),
            ipv4_subnet: Some("192.168.64.0/24".into()),
            builtin: true,
            attached: vec![("qtest".into(), Some("192.168.64.2/24".into()))],
            created: None,
        });
        s.clamp_selection();
        s
    }

    fn tel_sample() -> TelemetrySample {
        TelemetrySample {
            cpu: Some(33.0),
            mem: Some(42.0),
            rx: Some(12_288),
            tx: Some(4_096),
            r: Some(2_048),
            w: Some(8_192),
        }
    }

    fn with_telemetry(mut s: AppState, tel: VecDeque<TelemetrySample>) -> AppState {
        s.containers[0].telemetry = tel;
        s.containers[0].cpu_percent = Some(12.4);
        s.containers[0].mem_bytes = Some(48_000_000);
        s
    }

    fn render_theme(w: u16, h: u16, state: &AppState, ascii: bool) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        let th = Theme {
            truecolor: false,
            ascii,
        };
        terminal
            .draw(|f| {
                draw(f, state, &th);
            })
            .unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    fn render(w: u16, h: u16, state: &AppState) -> String {
        render_theme(w, h, state, true)
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
    fn no_terminal_size_panics_the_renderer() {
        let zoomed = || {
            let mut s = sample();
            s.zoom = true;
            s
        };
        let logs = || {
            let mut s = sample();
            s.focus = Focus::Detail;
            s.log_lines = (0..50).map(|i| format!("line {i}")).collect();
            s
        };
        let telemetry = || {
            let mut tel = VecDeque::new();
            for _ in 0..40 {
                tel.push_front(tel_sample());
            }
            with_telemetry(sample(), tel)
        };
        let splash = || {
            let mut s = sample();
            s.screen = Screen::Splash;
            s.first_run = true;
            s
        };
        let service_down = || {
            let mut s = sample();
            s.screen = Screen::ServiceDown;
            s
        };
        let states: [&dyn Fn() -> AppState; 6] =
            [&sample, &zoomed, &logs, &telemetry, &splash, &service_down];
        let overlays = [
            Overlay::None,
            Overlay::ActionMenu,
            Overlay::Help,
            Overlay::MessageLog,
            Overlay::PullInput {
                text: "alpine:latest".into(),
            },
            Overlay::TagInput {
                text: "myapp:v1".into(),
            },
            Overlay::CreateVolumeInput {
                text: "scratch".into(),
            },
            Overlay::Confirm {
                command: "container delete qtest".into(),
                action: crate::engine::state::ActionKind::DeleteContainer,
                target: "qtest".into(),
            },
        ];
        for build in states {
            for overlay in &overlays {
                let mut s = build();
                s.overlay = overlay.clone();
                for h in [1, 2, 3, 4, 5, 8, 11, 12, 17, 20, 22, 23, 30] {
                    for w in [1, 2, 10, 20, 40, 55, 60, 61, 79, 80, 100] {
                        render(w, h, &s);
                    }
                }
            }
        }
    }

    #[test]
    fn floor_55x20_keeps_all_four_panes_and_drops_floor_chrome() {
        let frame = render(55, 20, &sample());
        let header = frame.lines().next().unwrap();
        assert!(header.contains("1 containers"), "{header}");
        assert!(header.contains("2 images"), "{header}");
        assert!(header.contains("3 volumes"), "{header}");
        assert!(
            header.contains(" 4"),
            "55-col header keeps the networks key even if the title clips: {header}"
        );
        assert!(
            !header.contains("containers 1"),
            "counts live on the rail, not the header: {header}"
        );
        let second = frame.lines().nth(1).unwrap();
        assert!(
            second.contains("containers"),
            "expected 1-row header then rail, got {second:?}"
        );
        assert!(frame.contains("containers 1"), "{frame}");
        assert!(frame.contains("2 images 1"), "{frame}");
        assert!(frame.contains("3 volumes 1"), "{frame}");
        assert!(frame.contains("4 networks 1"), "{frame}");
        assert!(
            !frame.contains("name"),
            "no table headers at the floor: {frame}"
        );
        assert!(!frame.contains("Logs [l]"), "{frame}");
        assert!(!frame.contains("Inspect [i]"), "{frame}");
        assert!(!frame.contains("service"), "{frame}");
        assert!(frame.contains("cpu"), "{frame}");
        assert!(frame.contains("dsk r"), "{frame}");
    }

    #[test]
    fn wide_200x50_keeps_the_rail_cap() {
        let frame = render(200, 50, &sample());
        assert!(frame.contains("containers 1"), "{frame}");
        assert!(frame.contains("images 1"), "{frame}");
        assert!(frame.contains("volumes 1"), "{frame}");
        assert!(frame.contains("networks 1"), "{frame}");
        assert!(frame.contains("Logs [l]"), "{frame}");
        assert!(frame.contains("service"), "{frame}");
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
        assert!(frame.contains("networks 1"), "{frame}");
        assert!(frame.contains("Logs [l]"), "{frame}");
        assert!(frame.contains("service"), "{frame}");
        assert!(frame.contains("cpu"), "{frame}");
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
        assert!(!frame.contains("networks 1"), "{frame}");
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
        assert!(!frame.contains("networks 1"), "{frame}");
    }

    #[test]
    fn images_volumes_and_networks_have_inspect_only() {
        for pane in [Pane::Images, Pane::Volumes, Pane::Networks] {
            let mut s = sample();
            s.pane = pane;
            let frame = render(100, 30, &s);
            assert!(frame.contains(&format!("{} 1", pane.title())), "{frame}");
            assert!(!frame.contains("Logs [l]"), "{frame}");
            assert!(!frame.contains("dsk r"), "{frame}");
            assert!(!frame.contains("net ^"), "{frame}");
        }
    }

    #[test]
    fn expanding_a_pane_keeps_the_others_on_the_rail() {
        let mut s = sample();
        s.pane = Pane::Images;
        let frame = render(55, 20, &s);
        assert!(frame.contains("images 1"), "{frame}");
        assert!(frame.contains("1 containers 1"), "{frame}");
        assert!(frame.contains("3 volumes 1"), "{frame}");
        assert!(frame.contains("4 networks 1"), "{frame}");
        assert!(!frame.contains("Logs [l]"), "{frame}");
        assert!(!frame.contains("dsk r"), "{frame}");
    }

    #[test]
    fn strip_on_logs_and_inspect_shows_three_rows() {
        let mut tel = VecDeque::new();
        tel.push_front(tel_sample());
        let logs = with_telemetry(sample(), tel.clone());
        let mut inspect = with_telemetry(sample(), tel);
        inspect.detail_tab = DetailTab::Inspect;
        for view in [
            render_theme(100, 30, &logs, false),
            render_theme(100, 30, &inspect, false),
        ] {
            assert!(view.contains("cpu"), "{view}");
            assert!(view.contains("33.0%"), "{view}");
            assert!(view.contains("42%"), "{view}");
            assert!(view.contains("net ↑"), "{view}");
            assert!(view.contains("dsk r"), "{view}");
            assert!(view.contains("12.0K/s"), "{view}");
        }
    }

    #[test]
    fn sparks_auto_scale_to_the_visible_window() {
        let mut tel = VecDeque::new();
        let high = TelemetrySample {
            cpu: Some(90.0),
            mem: Some(90.0),
            rx: Some(0),
            tx: Some(0),
            r: Some(0),
            w: Some(0),
        };
        let low = TelemetrySample {
            cpu: Some(10.0),
            mem: Some(10.0),
            rx: Some(0),
            tx: Some(0),
            r: Some(0),
            w: Some(0),
        };
        for _ in 0..200 {
            tel.push_back(high);
        }
        for _ in 0..100 {
            tel.push_front(low);
        }
        let s = with_telemetry(sample(), tel);
        let view = render_theme(100, 30, &s, false);
        assert!(view.contains("10.0%"), "{view}");
        let cpu_line = view
            .lines()
            .find(|l| l.contains("cpu") && l.contains("10.0%"))
            .unwrap_or("");
        assert!(
            cpu_line.contains('█'),
            "visible window should fill: {cpu_line}"
        );
        assert!(
            !cpu_line.contains('▁'),
            "old 90% must not flatten the window: {cpu_line}"
        );
    }

    #[test]
    fn spark_number_is_the_true_percent_even_above_100() {
        let mut tel = VecDeque::new();
        tel.push_front(TelemetrySample {
            cpu: Some(150.0),
            mem: Some(42.0),
            rx: Some(0),
            tx: Some(0),
            r: Some(0),
            w: Some(0),
        });
        let s = with_telemetry(sample(), tel);
        let view = render_theme(100, 30, &s, false);
        assert!(view.contains("150.0%"), "{view}");
    }

    #[test]
    fn strip_yields_when_the_detail_inner_is_short() {
        let mut tel = VecDeque::new();
        tel.push_front(tel_sample());
        let s = with_telemetry(sample(), tel);
        let view = render_theme(80, 10, &s, false);
        assert!(!view.contains("dsk r"), "strip should collapse: {view}");
    }

    #[test]
    fn empty_and_stopped_render_as_dash() {
        let running_empty = sample();
        let view = render_theme(100, 30, &running_empty, false);
        assert!(view.contains("net ↑-"), "{view}");

        let mut tel = VecDeque::new();
        tel.push_front(tel_sample());
        let mut stopped = with_telemetry(sample(), tel);
        stopped.containers[0].state = "stopped".into();
        let view = render_theme(100, 30, &stopped, false);
        assert!(view.contains("net ↑-"), "{view}");
        assert!(
            !view.contains("33.0%"),
            "stopped current value is -: {view}"
        );
    }

    #[test]
    fn ascii_mode_uses_the_ramp_and_ascii_arrows() {
        let mut tel = VecDeque::new();
        tel.push_front(tel_sample());
        tel.push_front(TelemetrySample {
            cpu: Some(90.0),
            mem: Some(10.0),
            rx: Some(100),
            tx: Some(100),
            r: Some(100),
            w: Some(100),
        });
        let s = with_telemetry(sample(), tel);
        let view = render_theme(100, 30, &s, true);
        assert!(view.contains("net ^"), "{view}");
        assert!(
            view.contains('#') || view.contains('*') || view.contains('+'),
            "ascii ramp glyphs: {view}"
        );
        assert!(!view.contains('↑'), "{view}");
    }

    #[test]
    fn list_cpu_and_mem_stay_numeric() {
        let mut tel = VecDeque::new();
        tel.push_front(tel_sample());
        let s = with_telemetry(sample(), tel);
        let view = render_theme(100, 30, &s, false);
        assert!(view.contains("12.4%"), "{view}");
        assert!(view.contains("33.0%"), "{view}");
        assert!(view.contains("42%"), "{view}");
        assert!(!view.contains("mem  12.4"), "{view}");
    }

    fn confirming(command: &str) -> AppState {
        let mut s = sample();
        s.overlay = Overlay::Confirm {
            command: command.into(),
            action: crate::engine::ActionKind::DeleteContainer,
            target: "qtest".into(),
        };
        s
    }

    fn box_origin(frame: &str, title: &str) -> (usize, usize) {
        let head = format!("╭ {title} ");
        frame
            .lines()
            .enumerate()
            .find_map(|(y, line)| line.find(&head).map(|b| (y, line[..b].chars().count())))
            .unwrap_or_else(|| panic!("no {title} box in:\n{frame}"))
    }

    fn box_rows(frame: &str, title: &str) -> Vec<Vec<char>> {
        let grid: Vec<Vec<char>> = frame.lines().map(|l| l.chars().collect()).collect();
        let (top, left) = box_origin(frame, title);
        let right = grid[top][left..]
            .iter()
            .position(|&c| c == '╮')
            .map(|i| left + i)
            .unwrap_or_else(|| panic!("unterminated {title} box in:\n{frame}"));
        let mut rows = Vec::new();
        for row in grid.into_iter().skip(top) {
            let slice: Vec<char> = row.into_iter().skip(left).take(right - left + 1).collect();
            let last = slice.last().copied();
            rows.push(slice);
            if last == Some('╯') {
                break;
            }
        }
        rows
    }

    #[test]
    fn confirm_is_a_7_row_box_that_wraps_a_long_command_instead_of_growing() {
        let short = render(80, 24, &confirming("container delete qtest"));
        let long = render(
            80,
            24,
            &confirming("container delete a-really-long-container-name-that-overflows"),
        );
        for frame in [&short, &long] {
            let rows = box_rows(frame, "confirm");
            assert_eq!(rows.len(), 7, "confirm must stay 7 rows:\n{frame}");
            assert_eq!(
                rows[0].len(),
                48,
                "confirm must not grow to the command:\n{frame}"
            );
            assert!(frame.contains("[y] run"), "{frame}");
            assert!(frame.contains("[esc] cancel"), "{frame}");
        }
        assert!(
            long.contains("container delete a-really-long-container-n"),
            "{long}"
        );
        assert!(long.contains("ame-that-overflows"), "{long}");
    }

    #[test]
    fn confirm_still_fits_and_wraps_at_the_floor() {
        let frame = render(
            55,
            20,
            &confirming("container delete a-really-long-container-name-that-overflows"),
        );
        assert_eq!(box_rows(&frame, "confirm").len(), 7, "{frame}");
        assert!(frame.contains("ame-that-overflows"), "{frame}");
        assert!(frame.contains("[y] run"), "{frame}");
    }

    #[test]
    fn the_action_sheet_covers_the_detail_pane_only_never_the_rail() {
        let mut s = sample();
        s.overlay = Overlay::ActionMenu;
        for (w, h) in [(55u16, 20u16), (100, 30), (200, 50)] {
            let frame = render(w, h, &s);
            let sheet = box_rows(&frame, "actions");
            let (sheet_y, sheet_x) = box_origin(&frame, "actions");
            let (_, detail_x) = box_origin(&frame, "detail");
            assert!(
                sheet.len() as u16 <= layout::SHEET_MAX_H,
                "sheet is capped at 9 rows at {w}x{h}:\n{frame}"
            );
            assert_eq!(sheet_x, detail_x, "{w}x{h}:\n{frame}");
            assert_eq!(
                sheet[0].len(),
                box_rows(&frame, "detail")[0].len(),
                "{w}x{h}:\n{frame}"
            );
            assert!(frame.contains("containers 1"), "{w}x{h}:\n{frame}");
            assert!(frame.contains("images 1"), "{w}x{h}:\n{frame}");
            assert!(frame.contains("volumes 1"), "{w}x{h}:\n{frame}");
            assert!(frame.contains("networks 1"), "{w}x{h}:\n{frame}");
            if w < 80 {
                let rail_bottom = box_origin(&frame, "detail").0;
                assert!(
                    sheet_y >= rail_bottom,
                    "sheet at row {sheet_y} covers the rail (detail starts at {rail_bottom}):\n{frame}"
                );
            }
        }
    }

    #[test]
    fn the_floor_sheet_omits_l_and_i_but_lists_every_other_action() {
        let mut s = sample();
        s.overlay = Overlay::ActionMenu;
        let frame = render(55, 20, &s);
        for label in [
            "stop",
            "restart",
            "kill",
            "delete",
            "prune stopped",
            "exec shell",
        ] {
            assert!(frame.contains(label), "missing {label}:\n{frame}");
        }
        assert!(!frame.contains("  l  logs"), "{frame}");
        assert!(!frame.contains("  i  inspect"), "{frame}");
    }

    #[test]
    fn off_the_floor_the_sheet_still_lists_the_logs_jump() {
        let mut s = sample();
        s.overlay = Overlay::ActionMenu;
        let frame = render(100, 30, &s);
        assert!(frame.contains("l  logs"), "{frame}");
        assert!(frame.contains("Inspect [i]"), "{frame}");
    }

    #[test]
    fn a_command_too_long_even_to_wrap_is_marked_not_silently_cut() {
        let long = "container delete ".to_string() + &"x".repeat(400);
        let frame = render(80, 24, &confirming(&long));
        assert_eq!(box_rows(&frame, "confirm").len(), 7, "{frame}");
        assert!(frame.contains('…'), "clipped preview must say so:\n{frame}");
    }

    #[test]
    fn help_is_one_cheatsheet_clamped_to_the_frame_and_scrollable() {
        let floor = render(55, 20, &{
            let mut s = sample();
            s.overlay = Overlay::Help;
            s
        });
        let rows = box_rows(&floor, "keys");
        assert_eq!(rows.len(), 20, "{floor}");
        assert_eq!(rows[0].len(), 55, "{floor}");
        assert!(
            floor.contains("expand pane (containers / images / volu"),
            "{floor}"
        );
        assert!(
            floor.contains("mes / networks)") || floor.contains("networks)"),
            "{floor}"
        );
        assert!(floor.contains("j/k scroll"), "{floor}");
        let mut scrolled = sample();
        scrolled.overlay = Overlay::Help;
        scrolled.help_scroll = 6;
        let end = render(55, 20, &scrolled);
        assert!(end.contains("back to list"), "{end}");
        assert!(
            !end.contains("dismiss version banner") || end.contains("esc"),
            "{end}"
        );
        let mut roomy = sample();
        roomy.overlay = Overlay::Help;
        let wide = render(100, 30, &roomy);
        assert!(
            wide.contains("expand pane (containers / images / volumes / networks)"),
            "{wide}"
        );
        assert!(wide.contains("back to list"), "{wide}");
        assert!(!wide.contains("j/k scroll"), "{wide}");
    }

    #[test]
    fn networks_inspect_shows_attachment_gist_and_no_strip() {
        let mut s = sample();
        s.pane = Pane::Networks;
        s.inspect_cache.insert(
            "default".into(),
            "[\n  {\n    \"id\": \"default\"\n  }\n]".into(),
        );
        let frame = render(100, 30, &s);
        assert!(frame.contains("containers on this network"), "{frame}");
        assert!(frame.contains("qtest"), "{frame}");
        assert!(frame.contains("192.168.64.2/24"), "{frame}");
        assert!(frame.contains("\"default\""), "{frame}");
        assert!(!frame.contains("Logs [l]"), "{frame}");
        assert!(!frame.contains("dsk r"), "{frame}");
        assert!(!frame.contains("  s  stop"), "{frame}");

        s.zoom = true;
        s.focus = Focus::List;
        let zoomed = render(100, 30, &s);
        assert!(zoomed.contains("nat"), "{zoomed}");
        assert!(zoomed.contains("192.168.64.0/24"), "{zoomed}");
        assert!(zoomed.contains("builtin"), "{zoomed}");
    }

    #[test]
    fn pull_input_and_the_message_log_are_unchanged_at_the_floor() {
        let mut pull = sample();
        pull.pane = Pane::Images;
        pull.overlay = Overlay::PullInput {
            text: "alpine".into(),
        };
        let frame = render(55, 20, &pull);
        assert!(frame.contains("pull image"), "{frame}");
        assert!(frame.contains("reference: alpine"), "{frame}");

        let mut tag = sample();
        tag.pane = Pane::Images;
        tag.overlay = Overlay::TagInput {
            text: "myapp:v1".into(),
        };
        let frame = render(55, 20, &tag);
        assert!(frame.contains("tag image"), "{frame}");
        assert!(frame.contains("new reference: myapp:v1"), "{frame}");

        let mut log = sample();
        log.messages.push("boom: it failed".into());
        log.overlay = Overlay::MessageLog;
        let frame = render(55, 20, &log);
        assert!(frame.contains("message log"), "{frame}");
        assert!(frame.contains("boom: it failed"), "{frame}");
    }

    #[test]
    fn create_volume_dialog_is_name_only() {
        let mut s = sample();
        s.pane = Pane::Volumes;
        s.overlay = Overlay::CreateVolumeInput {
            text: "scratch2".into(),
        };
        let frame = render(80, 24, &s);
        assert!(frame.contains("create volume"), "{frame}");
        assert!(frame.contains("name: scratch2"), "{frame}");
        assert!(!frame.contains("driver"), "{frame}");
        assert!(!frame.contains("label"), "{frame}");
        assert!(!frame.contains("size"), "{frame}");
    }
}

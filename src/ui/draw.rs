use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};

use crate::config::LayoutMode;
use crate::engine::state::{AppState, DetailTab, Focus, Overlay, Pane, Screen, Setting};
use crate::ui::help::{HELP, HELP_KEY_COL};
use crate::ui::layout::{self, LayoutFacts, LayoutPlan, centered};
use crate::ui::rows::{absent, age_cell, state_dot, uptime_cell};
use crate::ui::theme::{ACCENT_A, ACCENT_B, Theme};
use crate::ui::{log_view, rail, strip, table};

const STRIP_MIN_LOG: u16 = 4;

#[derive(Debug, Default, Clone, Copy)]
pub struct DrawInfo {
    pub body: Rect,
    pub header: Rect,
    pub bottom: Rect,
    pub log_scroll: u16,
    pub help_max_scroll: u16,
}

pub(crate) fn spinner_frame() -> usize {
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

    draw_header(frame, state, th, plan.header, plan.floor);
    for (i, b) in banners.into_iter().enumerate() {
        let area = Rect {
            y: plan.banners.y + i as u16,
            height: 1,
            ..plan.banners
        };
        frame.render_widget(Paragraph::new(b), area);
    }

    let compact = plan.mode == LayoutMode::Table;
    if plan.zoom {
        match state.focus {
            Focus::List => match plan.mode {
                LayoutMode::Rail => rail::draw_zoomed(frame, state, th, plan.body),
                LayoutMode::Table => table::draw_zoomed(frame, state, th, plan.body),
            },
            Focus::Detail => draw_detail(frame, state, th, plan.body, info, plan.floor, compact),
        }
    } else {
        match plan.mode {
            LayoutMode::Rail => rail::draw(frame, state, th, &plan),
            LayoutMode::Table => table::draw(frame, state, th, &plan),
        }
        draw_detail(frame, state, th, plan.detail, info, plan.floor, compact);
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
        Overlay::Settings { cursor } => draw_settings(frame, state, th, *cursor),
        Overlay::None => {}
    }
}

fn draw_header(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect, floor: bool) {
    let mut spans = th.gradient_spans(" bushel ", true);
    spans.push(Span::raw("  "));
    match state.layout() {
        LayoutMode::Table => spans.extend(table::header_line(state, th)),
        LayoutMode::Rail => {
            for (i, pane) in Pane::all().into_iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw("   "));
                }
                spans.push(Span::styled(
                    format!("[{}]", pane.key()),
                    Style::new().fg(th.dim()),
                ));
                spans.push(Span::styled(
                    format!(" {}", pane.title()),
                    if state.pane == pane {
                        Style::new().fg(th.accent()).bold()
                    } else {
                        Style::new().fg(th.dim())
                    },
                ));
            }
        }
    }
    if !floor {
        append_status_cluster(&mut spans, state, th, area.width);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn append_status_cluster(spans: &mut Vec<Span<'static>>, state: &AppState, th: &Theme, width: u16) {
    let service_up = state.screen != Screen::ServiceDown;
    let version = state.cli_version.clone().unwrap_or_else(|| "?".into());
    let sp = th.spinner(spinner_frame());
    let cluster = format!("● service   container {version}  {sp} ");
    let cluster_len = cluster.chars().count();
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    if (width as usize) <= used + cluster_len {
        return;
    }
    spans.push(Span::raw(" ".repeat((width as usize) - used - cluster_len)));
    spans.push(Span::styled(
        if th.ascii { "* " } else { "● " },
        Style::new().fg(if service_up { th.accent() } else { th.red() }),
    ));
    spans.push(Span::styled("service   ", Style::new().fg(th.dim())));
    spans.push(Span::styled(
        format!("container {version}  "),
        Style::new().fg(th.dim()),
    ));
    spans.push(Span::styled(sp.to_string(), Style::new().fg(th.dim())));
    spans.push(Span::raw(" "));
}

fn detail_header(state: &AppState, th: &Theme, width: u16) -> Line<'static> {
    let mut left: Vec<Span> = vec![Span::raw(" ")];
    match state.pane {
        Pane::Containers => match state.selected_container() {
            Some(c) => {
                left.push(state_dot(th, c.is_running()));
                left.push(Span::styled(
                    c.id.clone(),
                    Style::new().fg(th.text()).bold(),
                ));
                left.push(Span::styled(
                    format!("   {} ", c.state),
                    Style::new().fg(if c.is_running() {
                        th.accent()
                    } else {
                        th.dim()
                    }),
                ));
                left.push(Span::styled(uptime_cell(th, c), Style::new().fg(th.dim())));
            }
            None => left.push(Span::styled("no selection", Style::new().fg(th.dim()))),
        },
        Pane::Images => match state.selected_image() {
            Some(im) => {
                left.push(Span::styled(
                    im.reference.clone(),
                    Style::new().fg(th.text()).bold(),
                ));
                let users: Vec<&str> = state
                    .containers
                    .iter()
                    .filter(|c| c.image == im.reference)
                    .map(|c| c.id.as_str())
                    .collect();
                let tail = if users.is_empty() {
                    "   image · unused".to_string()
                } else {
                    format!("   image · used by {}", users.join(", "))
                };
                left.push(Span::styled(tail, Style::new().fg(th.dim())));
            }
            None => left.push(Span::styled("no selection", Style::new().fg(th.dim()))),
        },
        Pane::Volumes => match state.selected_volume() {
            Some(v) => {
                left.push(Span::styled(
                    v.name.clone(),
                    Style::new().fg(th.text()).bold(),
                ));
                let tail = if v.in_use() {
                    format!("   volume · in use by {}", v.in_use_by.join(", "))
                } else {
                    "   volume · free".to_string()
                };
                left.push(Span::styled(tail, Style::new().fg(th.dim())));
                left.push(Span::styled(
                    format!("   {}", age_cell(th, v.created.as_deref())),
                    Style::new().fg(th.dim()),
                ));
            }
            None => left.push(Span::styled("no selection", Style::new().fg(th.dim()))),
        },
        Pane::Networks => match state.selected_network() {
            Some(n) => {
                left.push(Span::styled(
                    n.name.clone(),
                    Style::new().fg(th.text()).bold(),
                ));
                let subnet = n.ipv4_subnet.clone().unwrap_or_else(|| absent(th).into());
                let builtin = if n.builtin { " · builtin" } else { "" };
                left.push(Span::styled(
                    format!("   {} {subnet}{builtin}", n.mode),
                    Style::new().fg(th.dim()),
                ));
            }
            None => left.push(Span::styled("no selection", Style::new().fg(th.dim()))),
        },
    }

    let right: Vec<Span> = if state.pane == Pane::Containers {
        let tab = |label: &'static str, key: &'static str, on: bool| {
            let style = if on {
                Style::new().fg(th.accent()).bold().underlined()
            } else {
                Style::new().fg(th.dim())
            };
            vec![
                Span::styled(label, style),
                Span::styled(format!(" {key}"), Style::new().fg(th.dim())),
            ]
        };
        let logs = state.detail_tab == DetailTab::Logs;
        let mut r = tab("logs", "l", logs);
        r.push(Span::raw("   "));
        r.extend(tab("inspect", "i", !logs));
        r.push(Span::raw(" "));
        r
    } else {
        vec![
            Span::styled("inspect", Style::new().fg(th.accent()).bold()),
            Span::styled(" i ", Style::new().fg(th.dim())),
        ]
    };

    let used: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let want: usize = right.iter().map(|s| s.content.chars().count()).sum();
    let mut spans = left;
    if (width as usize) > used + want {
        spans.push(Span::raw(" ".repeat((width as usize) - used - want)));
        spans.extend(right);
    }
    Line::from(spans)
}

fn rule(th: &Theme, width: u16) -> Line<'static> {
    let glyph = if th.ascii { "-" } else { "─" };
    Line::from(Span::styled(
        glyph.repeat(width as usize),
        Style::new().fg(th.dim()),
    ))
}

fn draw_detail(
    frame: &mut Frame,
    state: &AppState,
    th: &Theme,
    area: Rect,
    info: &mut DrawInfo,
    floor: bool,
    compact: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let inner = if area.width >= 4 {
        Rect {
            x: area.x + 1,
            width: area.width - 2,
            ..area
        }
    } else {
        area
    };

    if let Some(pull) = &state.pull {
        if state.pane == Pane::Images {
            let mut lines = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", th.spinner(spinner_frame())),
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
                    format!(" {l}"),
                    Style::new().fg(th.dim()),
                )));
            }
            frame.render_widget(Paragraph::new(lines), inner);
            return;
        }
    }

    let mut content_area = inner;
    if !floor && inner.height >= 5 {
        frame.render_widget(
            Paragraph::new(detail_header(state, th, inner.width)),
            Rect { height: 1, ..inner },
        );
        frame.render_widget(
            Paragraph::new(rule(th, inner.width)),
            Rect {
                y: inner.y + 1,
                height: 1,
                ..inner
            },
        );
        content_area = Rect {
            y: inner.y + 2,
            height: inner.height - 2,
            ..inner
        };
    }

    if state.pane == Pane::Containers {
        let strip_h = strip::height(content_area.width, compact);
        if content_area.height >= strip_h + STRIP_MIN_LOG {
            let parts = Layout::vertical([Constraint::Length(strip_h), Constraint::Min(1)])
                .split(content_area);
            strip::draw(frame, state, th, parts[0]);
            content_area = parts[1];
            if content_area.height > STRIP_MIN_LOG {
                frame.render_widget(
                    Paragraph::new(rule(th, content_area.width)),
                    Rect {
                        height: 1,
                        ..content_area
                    },
                );
                content_area = Rect {
                    y: content_area.y + 1,
                    height: content_area.height - 1,
                    ..content_area
                };
            }
        }
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

fn key_glyph(th: &Theme, key: &'static str) -> &'static str {
    if !th.ascii {
        return match key {
            "enter" => "⏎",
            "space" => "␣",
            other => other,
        };
    }
    key
}

fn draw_bottom_bar(frame: &mut Frame, state: &AppState, th: &Theme, area: Rect, floor: bool) {
    let hint_style = Style::new().fg(th.dim());
    let key_style = Style::new().fg(th.accent());
    let mut spans: Vec<Span> = Vec::new();
    let mut transient = false;
    if let Some(t) = &state.toast {
        transient = true;
        let style = if t.error {
            Style::new().fg(th.red()).bold()
        } else {
            Style::new().fg(th.accent())
        };
        spans.push(Span::styled(format!(" {}", t.text), style));
    } else if let Some(a) = &state.activity {
        transient = true;
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
                ("j/k", "move"),
                ("space", "actions"),
                ("/", "filter"),
                ("f", "zoom"),
            ]
        } else if state.focus == Focus::List {
            &[
                ("j/k", "move"),
                ("enter", "focus"),
                ("space", "actions"),
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
            spans.push(Span::styled(format!(" {}", key_glyph(th, k)), key_style));
            spans.push(Span::styled(format!(" {v} "), hint_style));
        }
    }

    if !floor && !transient {
        append_selection_actions(&mut spans, state, th, area.width);
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::new().bg(th.bar())),
        area,
    );
}

fn append_selection_actions(
    spans: &mut Vec<Span<'static>>,
    state: &AppState,
    th: &Theme,
    width: u16,
) {
    let Some(label) = state.selection_label() else {
        return;
    };
    let mut actions = state.selection_actions();
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();

    let tail = loop {
        if actions.is_empty() {
            return;
        }
        let mut tail: Vec<Span<'static>> = vec![Span::styled(
            format!("{label} · "),
            Style::new().fg(th.dim()),
        )];
        for a in &actions {
            tail.push(Span::styled(
                a.key.to_string(),
                Style::new().fg(if a.destructive { th.red() } else { th.accent() }),
            ));
            tail.push(Span::styled(
                format!(" {}  ", a.label),
                Style::new().fg(th.dim()),
            ));
        }
        let want: usize = tail.iter().map(|s| s.content.chars().count()).sum();
        if used + want <= width as usize {
            break tail;
        }
        actions.pop();
    };

    let want: usize = tail.iter().map(|s| s.content.chars().count()).sum();
    spans.push(Span::raw(" ".repeat((width as usize) - used - want)));
    spans.extend(tail);
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

fn draw_prompt(frame: &mut Frame, th: &Theme, title: &str, field: &str, text: &str, hint: &str) {
    let area = centered(frame.area(), 56, 4);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(th.accent()).bold(),
        ))
        .style(Style::new().bg(th.panel()));
    let lines = vec![
        Line::from(vec![
            Span::raw(format!(" {field}: ")),
            Span::styled(format!("{text}▏"), Style::new().fg(th.text())),
        ]),
        Line::from(Span::styled(format!(" {hint}"), Style::new().fg(th.dim()))),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_pull_input(frame: &mut Frame, th: &Theme, text: &str) {
    draw_prompt(
        frame,
        th,
        "pull image",
        "reference",
        text,
        "enter pulls (tag defaults to :latest) · esc cancels",
    );
}

fn draw_tag_input(frame: &mut Frame, th: &Theme, text: &str) {
    draw_prompt(
        frame,
        th,
        "tag image",
        "new reference",
        text,
        "enter continues to confirm · esc cancels",
    );
}

fn draw_create_volume_input(frame: &mut Frame, th: &Theme, text: &str) {
    draw_prompt(
        frame,
        th,
        "create volume",
        "name",
        text,
        "enter continues to confirm · esc cancels",
    );
}

fn draw_settings(frame: &mut Frame, state: &AppState, th: &Theme, cursor: usize) {
    let rows = Setting::ALL.len() as u16 + 4;
    let area = layout::settings_modal(frame.area(), rows);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(th.accent()))
        .title(Line::from(th.gradient_spans(" settings ", true)))
        .title_bottom(Line::from(Span::styled(
            " j/k move · enter toggles · esc closes ",
            Style::new().fg(th.dim()),
        )))
        .style(Style::new().bg(th.panel()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (i, setting) in Setting::ALL.into_iter().enumerate() {
        let on = i == cursor;
        let bar = if on {
            Span::styled(
                crate::ui::rows::select_bar(th),
                Style::new().fg(th.accent()),
            )
        } else {
            Span::raw(" ")
        };
        lines.push(Line::from(vec![
            bar,
            Span::styled(
                format!(" {:<18}", setting.label()),
                if on {
                    Style::new().fg(th.text()).bold()
                } else {
                    Style::new().fg(th.dim())
                },
            ),
            Span::styled(
                setting.value(&state.config).to_string(),
                Style::new().fg(th.accent()).bold(),
            ),
        ]));
    }
    lines.push(Line::raw(""));
    if let Some(setting) = Setting::ALL.get(cursor) {
        lines.push(Line::from(Span::styled(
            format!(" {}", setting.blurb(&state.config)),
            Style::new().fg(th.text()),
        )));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        format!(" saved to {}", crate::config::Config::DOC_PATH),
        Style::new().fg(th.dim()),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
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
    use crate::engine::state::{
        ContainerEntry, ImageEntry, NetworkEntry, TelemetrySample, VolumeEntry,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::VecDeque;

    pub(crate) fn sample() -> AppState {
        let mut s = AppState::new(true);
        s.cli_version = Some("1.2.0".into());
        s.containers.push(ContainerEntry {
            id: "qtest".into(),
            image: "alpine:latest".into(),
            state: "running".into(),
            created: None,
            started: None,
            cpus: None,
            mem_limit: Some(2 * (1 << 30)),
            volumes: vec!["qvol".into()],
            networks: vec![("default".into(), Some("192.168.64.2/24".into()))],
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
        s.images.push(ImageEntry {
            reference: "docker.io/library/python:3.12-slim".into(),
            size: Some(48_000_000),
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

    fn tabled() -> AppState {
        let mut s = sample();
        s.config.layout = LayoutMode::Table;
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
        render_theme(w, h, state, false)
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

    fn line(frame: &str, n: usize) -> &str {
        frame.lines().nth(n).unwrap_or("")
    }

    #[test]
    fn no_terminal_size_panics_the_renderer_in_either_layout() {
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
        let empty = || AppState::new(true);
        let states: [&dyn Fn() -> AppState; 7] = [
            &sample,
            &zoomed,
            &logs,
            &telemetry,
            &splash,
            &service_down,
            &empty,
        ];
        let overlays = [
            Overlay::None,
            Overlay::ActionMenu,
            Overlay::Help,
            Overlay::MessageLog,
            Overlay::Settings { cursor: 3 },
            Overlay::PullInput {
                text: "alpine:latest".into(),
            },
            Overlay::Confirm {
                command: "container delete qtest".into(),
                action: crate::engine::state::ActionKind::DeleteContainer,
                target: "qtest".into(),
            },
        ];
        let sizes = |s: &AppState| {
            for h in [1, 2, 3, 4, 5, 8, 11, 12, 17, 20, 22, 23, 30] {
                for w in [1, 2, 10, 20, 40, 55, 60, 61, 79, 80, 100, 200] {
                    render(w, h, s);
                }
            }
            render_theme(55, 20, s, true);
            render_theme(120, 40, s, true);
        };
        for mode in [LayoutMode::Rail, LayoutMode::Table] {
            for build in states {
                for overlay in &overlays {
                    let mut s = build();
                    s.overlay = overlay.clone();
                    s.config.layout = mode;
                    sizes(&s);
                }
            }
            for pane in Pane::all() {
                let mut s = sample();
                s.config.layout = mode;
                s.pane = pane;
                s.zoom = pane == Pane::Networks;
                sizes(&s);
            }
        }
    }

    #[test]
    fn the_rail_is_borderless_with_one_rule_and_counts_only_on_the_rail() {
        let frame = render(120, 40, &sample());
        let header = line(&frame, 0);
        assert!(header.contains("[1] containers"), "{header}");
        assert!(header.contains("[4] networks"), "{header}");
        assert!(
            !header.contains("containers 1"),
            "rail-mode header keys carry no counts: {header}"
        );
        assert!(
            header.contains("● service"),
            "the status cluster moved up: {header}"
        );
        assert!(header.contains("container 1.2.0"), "{header}");
        assert!(
            !frame.contains('╭'),
            "no boxes on the rail or detail:\n{frame}"
        );
        assert!(
            line(&frame, 2).contains("containers                      1"),
            "{frame}"
        );
        assert!(
            frame.contains("images                          2"),
            "{frame}"
        );
        assert!(
            frame.contains("volumes                         1"),
            "{frame}"
        );
        assert!(
            frame.contains("networks                        1"),
            "{frame}"
        );
        let rule_cols: Vec<usize> = frame
            .lines()
            .skip(2)
            .take(36)
            .filter_map(|l| l.chars().position(|c| c == '│'))
            .collect();
        assert!(
            rule_cols.iter().all(|&c| c == 35),
            "one vertical rule at column 35, on every body row: {rule_cols:?}\n{frame}"
        );
        assert!(line(&frame, 3).starts_with("▎● qtest"), "{frame}");
    }

    #[test]
    fn the_rail_collapses_registry_prefixes_and_pools_slack_at_the_bottom() {
        let frame = render(120, 40, &sample());
        assert!(frame.contains(" dh python:3.12-slim"), "{frame}");
        assert!(!frame.contains("docker.io"), "{frame}");
        assert!(
            frame.contains("    alpine:latest"),
            "a bare reference keeps the token column so names line up:\n{frame}"
        );
        assert!(
            line(&frame, 38).contains("48.0 MB reclaimable · [P] prune"),
            "the footer sits on the rail's last row:\n{frame}"
        );
        assert!(
            line(&frame, 20).trim_start().starts_with('│'),
            "the body's spare rows are empty rail, not a stretched section:\n{frame}"
        );
    }

    #[test]
    fn the_bottom_bar_lists_the_actions_valid_on_the_selection() {
        let frame = render(120, 40, &sample());
        let bar = line(&frame, 39);
        assert!(bar.contains("j/k move  ⏎ focus  ␣ actions"), "{bar}");
        assert!(
            bar.contains("qtest · s stop  r restart  K kill  d delete"),
            "{bar}"
        );
        assert!(!bar.contains("service"), "the cluster left the bar: {bar}");
        assert!(
            !bar.contains("l logs"),
            "tab jumps are chrome, not bar actions: {bar}"
        );

        let mut stopped = sample();
        stopped.containers[0].state = "stopped".into();
        let bar = render(120, 40, &stopped);
        assert!(
            bar.contains("qtest · s start  d delete  P prune stopped"),
            "{bar}"
        );

        let mut narrow = sample();
        narrow.containers[0].id = "a-very-long-container-name-indeed".into();
        let bar = render(80, 30, &narrow);
        let last = line(&bar, 29);
        assert!(last.chars().count() <= 80, "{last}");
        assert!(
            last.contains("f zoom"),
            "hints survive when actions do not fit: {last}"
        );
    }

    #[test]
    fn the_detail_pane_opens_with_the_selection_and_its_tabs() {
        let frame = render(120, 40, &sample());
        let head = line(&frame, 2);
        assert!(head.contains("● qtest   running"), "{head}");
        assert!(head.ends_with("logs l   inspect i"), "{head}");
        assert!(
            line(&frame, 3).contains("────"),
            "a rule under the header: {frame}"
        );

        let mut inspect = sample();
        inspect.detail_tab = DetailTab::Inspect;
        let frame = render(120, 40, &inspect);
        assert!(frame.contains("loading inspect"), "{frame}");
    }

    #[test]
    fn the_table_layout_is_one_wide_table_over_one_wide_detail() {
        let frame = render(120, 40, &tabled());
        let header = line(&frame, 0);
        assert!(
            header.contains("[1] containers 1"),
            "counts live in the switcher: {header}"
        );
        assert!(header.contains("[2] images 2"), "{header}");
        assert!(header.contains("● service"), "{header}");
        let cols = line(&frame, 2);
        assert!(cols.contains("name"), "{cols}");
        assert!(cols.contains("state"), "{cols}");
        assert!(cols.contains("up"), "{cols}");
        assert!(cols.contains("cpu"), "{cols}");
        assert!(cols.contains("mem"), "{cols}");
        assert!(
            cols.contains("image"),
            "at 120 the image column fits: {cols}"
        );
        assert!(
            !cols.contains("volumes"),
            "but volumes waits for 152: {cols}"
        );
        assert_eq!(
            line(&frame, 3).chars().count(),
            120,
            "the rule under the heading spans the whole terminal"
        );
        let row = line(&frame, 4);
        assert!(row.starts_with("▎ ● qtest"), "{row}");
        assert!(row.contains("running"), "{row}");
        assert!(row.contains("1.2%"), "{row}");
        assert!(
            row.contains("3.8M / 2.0G"),
            "usage against the ceiling: {row}"
        );
        assert!(row.contains("alpine:latest"), "{row}");
        assert!(
            !frame.contains('│'),
            "no vertical rule in the table layout:\n{frame}"
        );
        assert!(
            !frame.contains("images                          2"),
            "no rail sections:\n{frame}"
        );
        assert!(
            line(&frame, 6).contains("● qtest   running"),
            "the detail header sits under the gap row:\n{frame}"
        );
    }

    #[test]
    fn the_table_drops_columns_from_the_right_and_keeps_data_under_its_heading() {
        let frame = render(55, 20, &tabled());
        let cols = line(&frame, 1);
        assert!(cols.contains("name"), "{cols}");
        assert!(cols.contains("cpu"), "{cols}");
        assert!(!cols.contains("state"), "{cols}");
        let row = line(&frame, 2);
        assert!(
            !row.contains("running"),
            "a dropped column must not leak its data into the next one: {row}"
        );
        assert!(row.contains("1.2%"), "{row}");

        let wide = render(200, 50, &tabled());
        let cols = line(&wide, 2);
        for head in ["network", "volumes", "created"] {
            assert!(cols.contains(head), "spare width buys columns: {cols}");
        }
        let row = line(&wide, 4);
        assert!(row.contains("default"), "{row}");
        assert!(row.contains("qvol"), "{row}");
    }

    #[test]
    fn the_table_strip_is_one_row_when_the_width_allows() {
        let mut tel = VecDeque::new();
        tel.push_front(tel_sample());
        let s = with_telemetry(tabled(), tel.clone());
        let frame = render(120, 40, &s);
        let strip = line(&frame, 8);
        assert!(strip.contains("cpu 33.0%"), "{strip}");
        assert!(strip.contains("mem 46M"), "{strip}");
        assert!(strip.contains("net ↑ 12.0K/s"), "{strip}");
        assert!(
            strip.contains("dsk r 2.0K/s"),
            "one row carries all four: {strip}"
        );

        let rail = render(120, 40, &with_telemetry(sample(), tel));
        assert!(line(&rail, 4).contains("cpu 33.0%"), "{rail}");
        assert!(line(&rail, 4).contains("mem 46M / 2.0G"), "{rail}");
        assert!(
            line(&rail, 5).contains("net ↑ 12.0K/s"),
            "two rows beside the rail: {rail}"
        );
    }

    #[test]
    fn every_pane_has_a_table_in_table_mode() {
        for (pane, head) in [
            (Pane::Images, "reference"),
            (Pane::Volumes, "used by"),
            (Pane::Networks, "subnet"),
        ] {
            let mut s = tabled();
            s.pane = pane;
            let frame = render(120, 40, &s);
            assert!(line(&frame, 2).contains(head), "{pane:?}:\n{frame}");
            assert!(
                !frame.contains("logs l"),
                "{pane:?} has inspect only:\n{frame}"
            );
            assert!(!frame.contains("dsk r"), "{pane:?}:\n{frame}");
            assert!(
                line(&frame, 0).contains(&format!("{} ", pane.title())),
                "{frame}"
            );
        }
        let mut s = tabled();
        s.pane = Pane::Networks;
        let frame = render(120, 40, &s);
        assert!(frame.contains("nat"), "{frame}");
        assert!(frame.contains("192.168.64.0/24"), "{frame}");
        assert!(frame.contains("builtin"), "{frame}");
        assert!(frame.contains("containers on this network"), "{frame}");
    }

    #[test]
    fn floor_55x20_drops_chrome_in_both_layouts() {
        for mode in [LayoutMode::Rail, LayoutMode::Table] {
            let mut s = sample();
            s.config.layout = mode;
            let frame = render(55, 20, &s);
            assert_eq!(frame.lines().count(), 20);
            assert!(line(&frame, 0).contains("[1] containers"), "{frame}");
            assert!(!frame.contains("service"), "{mode:?}:\n{frame}");
            assert!(!frame.contains("logs l"), "{mode:?}:\n{frame}");
            assert!(frame.contains("cpu"), "{mode:?}:\n{frame}");
            assert!(frame.contains("dsk r"), "{mode:?}:\n{frame}");
            assert!(!frame.contains("? help"), "{mode:?}:\n{frame}");
        }
        let rail = render(55, 20, &sample());
        assert!(rail.contains(" 2 images 2"), "{rail}");
        assert!(rail.contains(" 3 volumes 1"), "{rail}");
        assert!(rail.contains(" 4 networks 1"), "{rail}");
        let rule = rail
            .lines()
            .position(|l| l.starts_with("───"))
            .unwrap_or_else(|| panic!("no rule between the stacked rail and detail:\n{rail}"));
        assert!(
            line(&rail, rule - 1).contains("4 networks 1"),
            "the rule sits directly under the last rail section:\n{rail}"
        );
    }

    #[test]
    fn zoom_fullscreens_one_side_in_both_layouts() {
        for mode in [LayoutMode::Rail, LayoutMode::Table] {
            let mut s = sample();
            s.config.layout = mode;
            s.zoom = true;
            s.focus = Focus::List;
            let frame = render(100, 30, &s);
            assert!(frame.contains("qtest"), "{frame}");
            assert!(!frame.contains("logs l"), "{mode:?}:\n{frame}");
            assert!(
                !frame.lines().any(|l| l.trim_start().starts_with("images")),
                "{mode:?}: the other sections are gone:\n{frame}"
            );

            s.focus = Focus::Detail;
            let frame = render(100, 30, &s);
            assert!(frame.contains("logs l"), "{mode:?}:\n{frame}");
            assert!(!frame.contains("alpine:latest"), "{mode:?}:\n{frame}");
        }
    }

    #[test]
    fn expanding_a_pane_keeps_the_others_on_the_rail() {
        let mut s = sample();
        s.pane = Pane::Images;
        let frame = render(120, 40, &s);
        assert!(
            frame.contains("containers                      1"),
            "{frame}"
        );
        assert!(line(&frame, 6).starts_with("▎   alpine:latest"), "{frame}");
        assert!(line(&frame, 2).contains("inspect i"), "{frame}");
        assert!(!frame.contains("logs l"), "{frame}");
        assert!(!frame.contains("dsk r"), "{frame}");
        assert!(
            line(&frame, 2).contains("alpine:latest   image · used by qtest"),
            "{frame}"
        );
    }

    #[test]
    fn an_empty_section_offers_the_key_that_fills_it() {
        let mut s = sample();
        s.volumes.clear();
        s.images.clear();
        s.clamp_selection();
        let frame = render(120, 40, &s);
        assert!(frame.contains("none · [c] create"), "{frame}");
        assert!(frame.contains("none · [u] pull"), "{frame}");
        assert!(frame.contains("nothing to reclaim"), "{frame}");
        s.config.layout = LayoutMode::Table;
        s.pane = Pane::Volumes;
        let frame = render(120, 40, &s);
        assert!(frame.contains("no volumes · [c] create one"), "{frame}");
    }

    #[test]
    fn a_filter_rides_on_the_section_label() {
        let mut s = sample();
        s.filter = "qt".into();
        s.filter_input = true;
        let frame = render(120, 40, &s);
        assert!(line(&frame, 2).contains("containers  /qt▏"), "{frame}");
        s.filter = "zzz".into();
        let frame = render(120, 40, &s);
        assert!(frame.contains(" no match"), "{frame}");
    }

    #[test]
    fn the_settings_panel_shows_every_config_field_and_where_it_goes() {
        let mut s = sample();
        s.overlay = Overlay::Settings { cursor: 1 };
        let frame = render(100, 30, &s);
        let rows = box_rows(&frame, "settings");
        assert_eq!(rows.len(), Setting::ALL.len() + 6, "{frame}");
        assert!(frame.contains("layout            rail"), "{frame}");
        assert!(
            frame.contains("▎ ascii glyphs      off"),
            "the cursor marks a row: {frame}"
        );
        assert!(frame.contains("reduced motion    off"), "{frame}");
        assert!(frame.contains("splash on launch  on"), "{frame}");
        assert!(
            frame.contains("no Unicode dots, sparks or spinners"),
            "the highlighted row explains itself in full: {frame}"
        );
        assert!(
            frame.contains("saved to ~/.config/bushel/config.toml"),
            "{frame}"
        );
        assert!(frame.contains("enter toggles"), "{frame}");

        s.config.layout = LayoutMode::Table;
        s.config.ascii = true;
        let frame = render_theme(100, 30, &s, true);
        assert!(frame.contains("layout            table"), "{frame}");
        assert!(frame.contains("| ascii glyphs      on"), "{frame}");
        assert!(
            frame.contains("[1] containers 1"),
            "the layout behind the panel follows: {frame}"
        );
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
        let view = render(100, 30, &s);
        let cpu_line = view.lines().find(|l| l.contains("cpu 10.0%")).unwrap_or("");
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
    fn strip_yields_when_the_detail_is_short_and_dashes_when_stopped() {
        let mut tel = VecDeque::new();
        tel.push_front(tel_sample());
        let s = with_telemetry(sample(), tel.clone());
        assert!(
            !render(80, 8, &s).contains("dsk r"),
            "strip should collapse"
        );

        let mut stopped = with_telemetry(sample(), tel);
        stopped.containers[0].state = "stopped".into();
        let view = render(100, 30, &stopped);
        assert!(view.contains("net ↑ ·"), "{view}");
        assert!(
            !view.contains("33.0%"),
            "stopped current value is ·: {view}"
        );
        assert!(view.contains("cpu ·"), "{view}");
    }

    #[test]
    fn ascii_mode_has_no_unicode_anywhere_on_the_main_screen() {
        let mut tel = VecDeque::new();
        tel.push_front(tel_sample());
        for mode in [LayoutMode::Rail, LayoutMode::Table] {
            let mut s = with_telemetry(sample(), tel.clone());
            s.config.layout = mode;
            let view = render_theme(120, 40, &s, true);
            assert!(view.contains("net ^"), "{view}");
            assert!(
                view.contains("| * qtest") || view.contains("|* qtest"),
                "{view}"
            );
            assert!(view.contains(" enter focus"), "{view}");
            for glyph in ['↑', '│', '▎', '●', '⏎', '␣'] {
                assert!(!view.contains(glyph), "{mode:?} leaked {glyph:?}:\n{view}");
            }
        }
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
            assert_eq!(rows[0].len(), 48, "{frame}");
            assert!(frame.contains("[y] run"), "{frame}");
            assert!(frame.contains("[esc] cancel"), "{frame}");
        }
        assert!(long.contains("ame-that-overflows"), "{long}");
        let floor = render(
            55,
            20,
            &confirming("container delete a-really-long-container-name-that-overflows"),
        );
        assert_eq!(box_rows(&floor, "confirm").len(), 7, "{floor}");
    }

    #[test]
    fn the_action_sheet_covers_the_detail_pane_only_in_both_layouts() {
        for mode in [LayoutMode::Rail, LayoutMode::Table] {
            let mut s = sample();
            s.config.layout = mode;
            s.overlay = Overlay::ActionMenu;
            for (w, h) in [(55u16, 20u16), (100, 30), (200, 50)] {
                let frame = render(w, h, &s);
                let sheet = box_rows(&frame, "actions");
                let (sheet_y, sheet_x) = box_origin(&frame, "actions");
                let facts = LayoutFacts::from_state(&s);
                let plan = LayoutPlan::compute(Rect::new(0, 0, w, h), facts);
                assert!(
                    sheet.len() as u16 <= layout::SHEET_MAX_H,
                    "{mode:?} {w}x{h}:\n{frame}"
                );
                assert_eq!(sheet_x as u16, plan.detail.x, "{mode:?} {w}x{h}:\n{frame}");
                assert_eq!(
                    sheet[0].len() as u16,
                    plan.detail.width,
                    "{mode:?} {w}x{h}:\n{frame}"
                );
                assert!(
                    sheet_y as u16 >= plan.detail.y,
                    "{mode:?} {w}x{h}: sheet at row {sheet_y} covers the list:\n{frame}"
                );
                assert!(frame.contains("qtest"), "{mode:?} {w}x{h}:\n{frame}");
                if w < 80 {
                    assert!(
                        !frame.contains("l  logs"),
                        "floor sheet drops the jumps:\n{frame}"
                    );
                }
            }
        }
    }

    #[test]
    fn help_is_one_cheatsheet_clamped_to_the_frame_and_scrollable() {
        let mut s = sample();
        s.overlay = Overlay::Help;
        let floor = render(55, 20, &s);
        let rows = box_rows(&floor, "keys");
        assert_eq!(rows.len(), 20, "{floor}");
        assert_eq!(rows[0].len(), 55, "{floor}");
        assert!(floor.contains("j/k scroll"), "{floor}");
        let wide = render(100, 30, &s);
        assert!(
            wide.contains("expand pane (containers / images / volumes / networks)"),
            "{wide}"
        );
        assert!(wide.contains("settings (layout"), "{wide}");
        assert!(!wide.contains("j/k scroll"), "{wide}");
    }

    #[test]
    fn prompts_and_the_message_log_are_unchanged_at_the_floor() {
        let mut pull = sample();
        pull.pane = Pane::Images;
        pull.overlay = Overlay::PullInput {
            text: "alpine".into(),
        };
        let frame = render(55, 20, &pull);
        assert!(frame.contains("pull image"), "{frame}");
        assert!(frame.contains("reference: alpine"), "{frame}");

        let mut tag = sample();
        tag.overlay = Overlay::TagInput {
            text: "myapp:v1".into(),
        };
        assert!(render(55, 20, &tag).contains("new reference: myapp:v1"));

        let mut vol = sample();
        vol.overlay = Overlay::CreateVolumeInput {
            text: "scratch2".into(),
        };
        let frame = render(80, 24, &vol);
        assert!(frame.contains("create volume"), "{frame}");
        assert!(frame.contains("name: scratch2"), "{frame}");

        let mut log = sample();
        log.messages.push("boom: it failed".into());
        log.overlay = Overlay::MessageLog;
        let frame = render(55, 20, &log);
        assert!(frame.contains("message log"), "{frame}");
        assert!(frame.contains("boom: it failed"), "{frame}");
    }
}

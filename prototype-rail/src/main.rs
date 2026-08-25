//! PROTOTYPE — throwaway. Do not merge.
//!
//! Question: does the always-present three-panel rail feel right in a real
//! terminal at every tier (55×20 / 100×30 / 200×50)?
//!
//! Layout model (settled at charting, numbers here are the starting proposal):
//!   - all three panels always present; inactive collapse
//!   - body width < 80 → rail above the detail pane; else beside it
//!   - rail width capped at 36; spare columns belong to logs
//!   - 55×20 drops table headers, the Logs/Inspect tab row, and a header line

use std::io::{self, stdout};
use std::path::Path;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Paragraph, Row, Table, TableState, Tabs};
use ratatui::{Frame, Terminal};

// ── starting numbers to react to ──────────────────────────────────────────

const STACK_BELOW: u16 = 80;
const RAIL_MAX: u16 = 36;
const RAIL_PCT: u16 = 45;
const TIGHT_RAIL_H: u16 = 16;

const PRESET_FLOOR: (u16, u16) = (55, 20);
const PRESET_MED: (u16, u16) = (100, 30);
const PRESET_WIDE: (u16, u16) = (200, 50);

const ACCENT_A: (u8, u8, u8) = (0x7e, 0xe7, 0x87);
const ACCENT_B: (u8, u8, u8) = (0xff, 0x7b, 0x72);

// ── theme (copied from bushel, prototype-local) ───────────────────────────

#[derive(Clone, Copy)]
struct Theme;

impl Theme {
    fn bg(self) -> Color {
        Color::Rgb(0x0f, 0x11, 0x17)
    }
    fn panel(self) -> Color {
        Color::Rgb(0x14, 0x17, 0x20)
    }
    fn bar(self) -> Color {
        Color::Rgb(0x11, 0x14, 0x1c)
    }
    fn highlight(self) -> Color {
        Color::Rgb(0x24, 0x2b, 0x3a)
    }
    fn dim(self) -> Color {
        Color::Rgb(0x5c, 0x63, 0x70)
    }
    fn text(self) -> Color {
        Color::Rgb(0xc9, 0xd1, 0xd9)
    }
    fn accent(self) -> Color {
        Color::Rgb(ACCENT_A.0, ACCENT_A.1, ACCENT_A.2)
    }
    fn red(self) -> Color {
        Color::Rgb(ACCENT_B.0, ACCENT_B.1, ACCENT_B.2)
    }
    fn yellow(self) -> Color {
        Color::Rgb(0xe3, 0xb3, 0x41)
    }
    fn chrome(self) -> Color {
        Color::Rgb(0x08, 0x09, 0x0d)
    }

    fn gradient_spans(self, text: &str) -> Vec<Span<'static>> {
        let n = text.chars().count().max(1);
        text.chars()
            .enumerate()
            .map(|(i, c)| {
                let t = i as f32 / (n - 1).max(1) as f32;
                let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
                Span::styled(
                    c.to_string(),
                    Style::new()
                        .fg(Color::Rgb(
                            f(ACCENT_A.0, ACCENT_B.0),
                            f(ACCENT_A.1, ACCENT_B.1),
                            f(ACCENT_A.2, ACCENT_B.2),
                        ))
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect()
    }
}

const TH: Theme = Theme;

// ── domain (fake) ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Containers,
    Images,
    Volumes,
}

impl Pane {
    fn index(self) -> usize {
        match self {
            Pane::Containers => 0,
            Pane::Images => 1,
            Pane::Volumes => 2,
        }
    }
    fn next(self) -> Pane {
        match self {
            Pane::Containers => Pane::Images,
            Pane::Images => Pane::Volumes,
            Pane::Volumes => Pane::Containers,
        }
    }
    fn title(self) -> &'static str {
        match self {
            Pane::Containers => "containers",
            Pane::Images => "images",
            Pane::Volumes => "volumes",
        }
    }
    fn key(self) -> char {
        match self {
            Pane::Containers => '1',
            Pane::Images => '2',
            Pane::Volumes => '3',
        }
    }
    fn all() -> [Pane; 3] {
        [Pane::Containers, Pane::Images, Pane::Volumes]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    List,
    Detail,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    Logs,
    Inspect,
}

struct Container {
    id: &'static str,
    image: &'static str,
    running: bool,
    cpu: f64,
    mem_m: u32,
}

struct Image {
    reference: &'static str,
    size: &'static str,
}

struct Volume {
    name: &'static str,
    in_use: bool,
}

struct State {
    pane: Pane,
    focus: Focus,
    tab: DetailTab,
    zoom: bool,
    filter: String,
    filter_input: bool,
    selected: [usize; 3],
    help: bool,
    /// None = fill the real terminal.
    preset: Option<(u16, u16)>,
    containers: Vec<Container>,
    images: Vec<Image>,
    volumes: Vec<Volume>,
    logs: Vec<String>,
}

fn seed() -> State {
    let containers = vec![
        Container {
            id: "redis",
            image: "redis:7",
            running: true,
            cpu: 1.2,
            mem_m: 48,
        },
        Container {
            id: "postgres",
            image: "postgres:16",
            running: true,
            cpu: 4.8,
            mem_m: 256,
        },
        Container {
            id: "caddy",
            image: "caddy:2",
            running: true,
            cpu: 0.3,
            mem_m: 12,
        },
        Container {
            id: "bushel-smoke",
            image: "alpine:latest",
            running: true,
            cpu: 0.1,
            mem_m: 4,
        },
        Container {
            id: "github-actions-runner-2",
            image: "ghcr.io/example/worker:1.4.2",
            running: true,
            cpu: 12.4,
            mem_m: 512,
        },
        Container {
            id: "worker-a",
            image: "alpine:latest",
            running: false,
            cpu: 0.0,
            mem_m: 0,
        },
        Container {
            id: "worker-b",
            image: "alpine:latest",
            running: false,
            cpu: 0.0,
            mem_m: 0,
        },
        Container {
            id: "old-migrate",
            image: "postgres:16",
            running: false,
            cpu: 0.0,
            mem_m: 0,
        },
    ];
    let images = vec![
        Image {
            reference: "docker.io/library/postgres:16",
            size: "431 MB",
        },
        Image {
            reference: "docker.io/library/redis:7",
            size: "116 MB",
        },
        Image {
            reference: "docker.io/library/caddy:2",
            size: "48 MB",
        },
        Image {
            reference: "docker.io/library/alpine:latest",
            size: "8.3 MB",
        },
        Image {
            reference: "ghcr.io/example/worker:1.4.2",
            size: "1.1 GB",
        },
    ];
    let volumes = vec![
        Volume {
            name: "pg-data",
            in_use: true,
        },
        Volume {
            name: "redis-data",
            in_use: true,
        },
        Volume {
            name: "leftover",
            in_use: false,
        },
    ];
    let logs = vec![
        "2026-08-24T18:02:11.441Z INFO  postgres  checkpoint complete: wrote 214 buffers (1.6%); 0 WAL file(s) added, 0 removed, 1 recycled; write=0.215 s, sync=0.031 s, total=0.251 s".into(),
        "2026-08-24T18:02:12.018Z INFO  redis     replica 192.168.64.4:6379 asks for sync, starting BGSAVE".into(),
        "2026-08-24T18:02:12.441Z INFO  caddy     192.168.64.1 - GET /health 200 1.2ms".into(),
        "2026-08-24T18:02:13.102Z WARN  postgres  could not serialize access due to concurrent update".into(),
        "2026-08-24T18:02:13.880Z INFO  runner    job 1842 queued: build / test (aarch64-apple-darwin)".into(),
        "2026-08-24T18:02:14.201Z INFO  redis     10000 changes in 60 seconds. Saving...".into(),
        "2026-08-24T18:02:14.990Z INFO  postgres  statement: SELECT * FROM orders WHERE created_at > now() - interval '5 minutes' AND status IN ('pending','paid')".into(),
        "2026-08-24T18:02:15.441Z INFO  caddy     192.168.64.8 - POST /webhooks/github 204 42.8ms".into(),
        "2026-08-24T18:02:16.002Z INFO  runner    cloning github.com/frankieramirez/bushel@frankieramirez/issue-14-wayfinder".into(),
        "2026-08-24T18:02:16.771Z ERROR postgres  FATAL:  remaining connection slots are reserved for non-replication superuser connections".into(),
        "2026-08-24T18:02:17.110Z INFO  redis     DB saved on disk".into(),
        "2026-08-24T18:02:17.880Z INFO  caddy     logger: flushed 128 entries".into(),
        "2026-08-24T18:02:18.441Z INFO  postgres  automatic vacuum of table \"public.orders\": index scans: 1, pages: 0 removed, 184 remain".into(),
        "2026-08-24T18:02:19.002Z INFO  runner    cargo test --offline".into(),
        "2026-08-24T18:02:19.660Z INFO  redis     client closed connection".into(),
        "2026-08-24T18:02:20.101Z INFO  postgres  duration: 812.441 ms  execute <unnamed>: SELECT n FROM generate_series(1,1000000) n".into(),
        "2026-08-24T18:02:20.880Z INFO  caddy     tls: remaining 2 certificates".into(),
        "2026-08-24T18:02:21.441Z INFO  runner    test engine::poll::confirms_pending ... ok".into(),
        "2026-08-24T18:02:22.018Z WARN  postgres  log line too long to read at 55 columns — this is the unreadability complaint".into(),
        "2026-08-24T18:02:22.441Z INFO  redis     1.2µs per op, 800k ops/s".into(),
    ];
    State {
        pane: Pane::Containers,
        focus: Focus::List,
        tab: DetailTab::Logs,
        zoom: false,
        filter: String::new(),
        filter_input: false,
        selected: [0, 0, 0],
        help: false,
        preset: None,
        containers,
        images,
        volumes,
        logs,
    }
}

fn fuzzy(needle: &str, hay: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let hay = hay.to_lowercase();
    let mut chars = hay.chars();
    needle
        .to_lowercase()
        .chars()
        .all(|n| chars.by_ref().any(|h| h == n))
}

impl State {
    fn visible(&self, pane: Pane) -> Vec<usize> {
        let f = if pane == self.pane {
            self.filter.as_str()
        } else {
            ""
        };
        match pane {
            Pane::Containers => self
                .containers
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    fuzzy(
                        f,
                        &format!(
                            "{} {} {}",
                            c.id,
                            c.image,
                            if c.running { "running" } else { "stopped" }
                        ),
                    )
                })
                .map(|(i, _)| i)
                .collect(),
            Pane::Images => self
                .images
                .iter()
                .enumerate()
                .filter(|(_, i)| fuzzy(f, i.reference))
                .map(|(i, _)| i)
                .collect(),
            Pane::Volumes => self
                .volumes
                .iter()
                .enumerate()
                .filter(|(_, v)| fuzzy(f, v.name))
                .map(|(i, _)| i)
                .collect(),
        }
    }

    fn move_sel(&mut self, delta: isize) {
        let rows = self.visible(self.pane);
        if rows.is_empty() {
            return;
        }
        let cur = self.selected[self.pane.index()].min(rows.len() - 1);
        let next = (cur as isize + delta).clamp(0, rows.len() as isize - 1) as usize;
        self.selected[self.pane.index()] = next;
    }

    fn selected_id(&self) -> String {
        let rows = self.visible(self.pane);
        let i = *rows.get(self.selected[self.pane.index()]).unwrap_or(&0);
        match self.pane {
            Pane::Containers => self.containers.get(i).map(|c| c.id.to_string()),
            Pane::Images => self.images.get(i).map(|im| im.reference.to_string()),
            Pane::Volumes => self.volumes.get(i).map(|v| v.name.to_string()),
        }
        .unwrap_or_else(|| "—".into())
    }
}

// ── layout ────────────────────────────────────────────────────────────────

struct Plan {
    header: Rect,
    body: Rect,
    bottom: Rect,
    rail: Rect,
    detail: Rect,
    slots: [Rect; 3],
    stacked: bool,
    tight: bool,
    compact: bool,
    zoom: bool,
}

impl Plan {
    fn compute(area: Rect, state: &State) -> Self {
        let compact = area.height <= 22 || area.width <= 60;
        let header_h = if compact { 1 } else { 2 };
        let chunks = Layout::vertical([
            Constraint::Length(header_h),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);
        let header = chunks[0];
        let body = chunks[1];
        let bottom = chunks[2];

        if state.zoom {
            return Self {
                header,
                body,
                bottom,
                rail: body,
                detail: body,
                slots: [body, Rect::default(), Rect::default()],
                stacked: false,
                tight: compact,
                compact,
                zoom: true,
            };
        }

        let stacked = body.width < STACK_BELOW;
        let tight = stacked || body.height < TIGHT_RAIL_H;

        let (rail, detail) = if stacked {
            let rail_h = stacked_rail_height(body.height, state);
            let parts =
                Layout::vertical([Constraint::Length(rail_h), Constraint::Min(3)]).split(body);
            (parts[0], parts[1])
        } else {
            let rail_w = (body.width * RAIL_PCT / 100).min(RAIL_MAX).max(18);
            let parts =
                Layout::horizontal([Constraint::Length(rail_w), Constraint::Min(12)]).split(body);
            (parts[0], parts[1])
        };

        let slots = rail_slots(rail, state, tight);
        Self {
            header,
            body,
            bottom,
            rail,
            detail,
            slots,
            stacked,
            tight,
            compact,
            zoom: false,
        }
    }

    fn summary(&self, state: &State) -> String {
        let mode = if self.zoom {
            "zoom"
        } else if self.stacked {
            "stacked"
        } else {
            "beside"
        };
        let collapse = if self.tight { "tight" } else { "roomy" };
        format!(
            "{mode}  rail {}×{}  detail {}×{}  collapse={collapse}  cap={RAIL_MAX}  stack<{STACK_BELOW}  log_cols={}",
            self.rail.width,
            self.rail.height,
            self.detail.width,
            self.detail.height,
            self.detail.width.saturating_sub(2),
        ) + &format!("  active={}", state.pane.title())
    }
}

fn stacked_rail_height(body_h: u16, state: &State) -> u16 {
    let inactive = 2u16; // two 1-row collapsed panels
    let rows = state.visible(state.pane).len() as u16;
    let active_h = (rows + 2).clamp(4, 8);
    let want = inactive + active_h;
    let cap = (body_h / 2).max(6);
    want.clamp(6, cap.min(body_h.saturating_sub(4)))
}

fn rail_slots(rail: Rect, state: &State, tight: bool) -> [Rect; 3] {
    let panes = Pane::all();
    let constraints: Vec<Constraint> = panes
        .iter()
        .map(|&p| {
            if p == state.pane {
                Constraint::Fill(1)
            } else if tight {
                Constraint::Length(1)
            } else {
                let need = state.visible(p).len() as u16 + 2;
                // floor of 8 so a handful of images/volumes aren't clipped
                // at medium height; /4 still caps a long inactive list
                let cap = (rail.height / 4).max(8);
                Constraint::Length(need.clamp(3, cap))
            }
        })
        .collect();
    let split = Layout::vertical(constraints).split(rail);
    [split[0], split[1], split[2]]
}

// ── draw ──────────────────────────────────────────────────────────────────

fn draw(frame: &mut Frame, state: &State, viewport: Option<(u16, u16)>) {
    let full = frame.area();
    frame.render_widget(
        Block::new().style(Style::new().bg(TH.bg()).fg(TH.text())),
        full,
    );

    let (app, chrome) = if let Some((w, h)) = viewport {
        let w = w.min(full.width);
        let h = h.min(full.height);
        let app = if full.height > h + 1 {
            Rect::new(
                full.x + (full.width - w) / 2,
                full.y + 1 + (full.height.saturating_sub(h + 1)) / 2,
                w,
                h,
            )
        } else {
            Rect::new(full.x + (full.width - w) / 2, full.y, w, h)
        };
        (app, true)
    } else {
        (full, false)
    };

    if chrome {
        let plan = Plan::compute(app, state);
        let clipped = viewport.is_some_and(|(w, h)| full.width < w || full.height < h);
        let (pw, ph) = viewport.unwrap();
        let clip = if clipped {
            "  (clipped to this terminal)"
        } else {
            ""
        };
        let line = format!(
            " PROTOTYPE  {pw}×{ph}{clip}  {}  F1 55×20  F2 100×30  F3 200×50  F4 live  ? help",
            plan.summary(state)
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                line,
                Style::new().fg(TH.yellow()).bg(TH.chrome()),
            ))),
            Rect::new(0, 0, full.width, 1),
        );
    }

    let plan = Plan::compute(app, state);
    draw_app(frame, state, &plan);

    if state.help {
        draw_help(frame, full);
    }
}

fn draw_app(frame: &mut Frame, state: &State, plan: &Plan) {
    frame.render_widget(
        Block::new().style(Style::new().bg(TH.bg()).fg(TH.text())),
        Rect::new(
            plan.header.x,
            plan.header.y,
            plan.header.width,
            plan.header.height + plan.body.height + plan.bottom.height,
        ),
    );
    draw_header(frame, state, plan.header, plan.compact);
    if plan.zoom {
        match state.focus {
            Focus::List => draw_list_pane(frame, state, state.pane, plan.body, false, plan.compact),
            Focus::Detail => draw_detail(frame, state, plan.body, plan.compact),
        }
    } else {
        for (i, pane) in Pane::all().into_iter().enumerate() {
            let tight = pane != state.pane && plan.tight;
            draw_list_pane(frame, state, pane, plan.slots[i], tight, plan.compact);
        }
        draw_detail(frame, state, plan.detail, plan.compact);
    }
    draw_bottom(frame, state, plan);
}

fn draw_header(frame: &mut Frame, state: &State, area: Rect, _compact: bool) {
    let mut spans = TH.gradient_spans(" bushel ");
    spans.push(Span::raw("  "));
    for (i, pane) in Pane::all().into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::new().fg(TH.dim())));
        }
        let style = if state.pane == pane {
            Style::new().fg(TH.accent()).bold().underlined()
        } else {
            Style::new().fg(TH.dim())
        };
        spans.push(Span::styled(
            format!("{} {}", pane.key(), pane.title()),
            style,
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn pane_block(title: &str, focused: bool, extra: Option<Line<'static>>) -> Block<'static> {
    let border = if focused {
        Style::new().fg(TH.accent())
    } else {
        Style::new().fg(TH.dim())
    };
    let mut b = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(
            format!(" {title} "),
            if focused {
                Style::new().fg(TH.accent()).bold()
            } else {
                Style::new().fg(TH.text())
            },
        ))
        .style(Style::new().bg(TH.panel()));
    if let Some(l) = extra {
        b = b.title_bottom(l);
    }
    b
}

fn draw_list_pane(
    frame: &mut Frame,
    state: &State,
    pane: Pane,
    area: Rect,
    tight: bool,
    compact: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let active = pane == state.pane;
    let focused = active && state.focus == Focus::List;
    let n = match pane {
        Pane::Containers => state.containers.len(),
        Pane::Images => state.images.len(),
        Pane::Volumes => state.volumes.len(),
    };

    if tight {
        let style_key = if active {
            Style::new().fg(TH.accent()).bold()
        } else {
            Style::new().fg(TH.dim())
        };
        let style_rest = if active {
            Style::new().fg(TH.text())
        } else {
            Style::new().fg(TH.dim())
        };
        let line = Line::from(vec![
            Span::styled(format!(" {} ", pane.key()), style_key),
            Span::styled(pane.title().to_string(), style_rest),
            Span::styled(format!(" {n}"), Style::new().fg(TH.text())),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(Style::new().bg(TH.panel())),
            area,
        );
        return;
    }

    let filter_line = if active && (state.filter_input || !state.filter.is_empty()) {
        let cursor = if state.filter_input { "▏" } else { "" };
        Some(Line::from(vec![
            Span::styled(" /", Style::new().fg(TH.accent()).bold()),
            Span::styled(
                format!("{}{cursor} ", state.filter),
                Style::new().fg(TH.text()),
            ),
        ]))
    } else {
        None
    };
    let title = format!("{} {n}", pane.title());
    let block = pane_block(&title, focused, filter_line);
    let inner_h = area.height.saturating_sub(2);
    let show_header = !compact && inner_h >= 6 && active;

    let rows_idx = state.visible(pane);
    let sel = state.selected[pane.index()].min(rows_idx.len().saturating_sub(1));
    let highlight = Style::new().bg(TH.highlight()).fg(TH.text()).bold();

    let (header, rows, widths) = match pane {
        Pane::Containers => {
            let wide = area.width >= 50 && active;
            let mid = area.width >= 32 && active;
            let rows: Vec<Row> = rows_idx
                .iter()
                .map(|&i| {
                    let c = &state.containers[i];
                    let dot = if c.running { "● " } else { "○ " };
                    let dot_s = Style::new().fg(if c.running { TH.accent() } else { TH.dim() });
                    let name = Line::from(vec![Span::styled(dot, dot_s), Span::raw(c.id)]);
                    let style = if c.running {
                        Style::new().fg(TH.text())
                    } else {
                        Style::new().fg(TH.dim())
                    };
                    if wide {
                        Row::new(vec![
                            Cell::from(name),
                            Cell::from(format!("{:>4.1}%", c.cpu)),
                            Cell::from(format!("{:>4}M", c.mem_m)),
                            Cell::from(c.image),
                        ])
                        .style(style)
                    } else if mid {
                        Row::new(vec![
                            Cell::from(name),
                            Cell::from(format!("{:>4.1}%", c.cpu)),
                            Cell::from(format!("{:>4}M", c.mem_m)),
                        ])
                        .style(style)
                    } else {
                        Row::new(vec![Cell::from(name)]).style(style)
                    }
                })
                .collect();
            if wide {
                (
                    Row::new(vec!["name", "cpu", "mem", "image"])
                        .style(Style::new().fg(TH.dim()).bold()),
                    rows,
                    vec![
                        Constraint::Min(14),
                        Constraint::Length(6),
                        Constraint::Length(6),
                        Constraint::Min(10),
                    ],
                )
            } else if mid {
                (
                    Row::new(vec!["name", "cpu", "mem"]).style(Style::new().fg(TH.dim()).bold()),
                    rows,
                    vec![
                        Constraint::Min(14),
                        Constraint::Length(6),
                        Constraint::Length(6),
                    ],
                )
            } else {
                (
                    Row::new(vec!["name"]).style(Style::new().fg(TH.dim()).bold()),
                    rows,
                    vec![Constraint::Min(10)],
                )
            }
        }
        Pane::Images => {
            let rows: Vec<Row> = rows_idx
                .iter()
                .map(|&i| {
                    let im = &state.images[i];
                    if active && area.width >= 40 {
                        Row::new(vec![Cell::from(im.reference), Cell::from(im.size)])
                    } else {
                        Row::new(vec![Cell::from(im.reference)])
                    }
                })
                .collect();
            if active && area.width >= 40 {
                (
                    Row::new(vec!["reference", "size"]).style(Style::new().fg(TH.dim()).bold()),
                    rows,
                    vec![Constraint::Min(16), Constraint::Length(8)],
                )
            } else {
                (
                    Row::new(vec!["reference"]).style(Style::new().fg(TH.dim()).bold()),
                    rows,
                    vec![Constraint::Min(10)],
                )
            }
        }
        Pane::Volumes => {
            let rows: Vec<Row> = rows_idx
                .iter()
                .map(|&i| {
                    let v = &state.volumes[i];
                    let badge = if v.in_use {
                        Span::styled("in use", Style::new().fg(TH.yellow()))
                    } else {
                        Span::styled("-", Style::new().fg(TH.dim()))
                    };
                    Row::new(vec![Cell::from(v.name), Cell::from(Line::from(badge))])
                })
                .collect();
            (
                Row::new(vec!["name", ""]).style(Style::new().fg(TH.dim()).bold()),
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

fn draw_detail(frame: &mut Frame, state: &State, area: Rect, compact: bool) {
    if area.height == 0 {
        return;
    }
    let focused = state.focus == Focus::Detail;
    let log_cols = area.width.saturating_sub(2);
    let extra = Some(Line::from(Span::styled(
        format!(" {log_cols} cols "),
        Style::new().fg(TH.dim()),
    )));
    let block = pane_block("detail", focused, extra);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut content = inner;
    let show_tabs = !compact && state.pane == Pane::Containers && inner.height >= 6;
    if show_tabs {
        let tabs_area = Rect { height: 1, ..inner };
        content = Rect {
            y: inner.y + 1,
            height: inner.height.saturating_sub(1),
            ..inner
        };
        let idx = if state.tab == DetailTab::Logs { 0 } else { 1 };
        frame.render_widget(
            Tabs::new(vec![" Logs [l] ", " Inspect [i] "])
                .select(idx)
                .style(Style::new().fg(TH.dim()))
                .highlight_style(Style::new().fg(TH.accent()).bold().underlined()),
            tabs_area,
        );
    }

    let lines = if state.pane == Pane::Containers && state.tab == DetailTab::Logs {
        log_lines(state)
    } else {
        let inspect = match state.pane {
            Pane::Containers => format!(
                "{{\n  \"id\": \"{}\",\n  \"image\": \"postgres:16\",\n  \"state\": \"running\"\n}}",
                state.selected_id()
            ),
            Pane::Images => format!("{{\n  \"reference\": \"{}\"\n}}", state.selected_id()),
            Pane::Volumes => format!("{{\n  \"name\": \"{}\"\n}}", state.selected_id()),
        };
        inspect
            .lines()
            .map(|l| Line::from(Span::styled(l.to_string(), Style::new().fg(TH.dim()))))
            .collect()
    };

    frame.render_widget(Paragraph::new(lines), content);
}

fn log_lines(state: &State) -> Vec<Line<'static>> {
    let mut l: Vec<Line> = state
        .logs
        .iter()
        .map(|s| {
            let style = if s.contains("ERROR") {
                Style::new().fg(TH.red())
            } else if s.contains("WARN") {
                Style::new().fg(TH.yellow())
            } else {
                Style::new().fg(TH.text())
            };
            Line::from(Span::styled(s.clone(), style))
        })
        .collect();
    l.push(Line::from(Span::styled(
        "── following (F to pause) ──",
        Style::new().fg(TH.accent()),
    )));
    l
}

fn draw_bottom(frame: &mut Frame, state: &State, plan: &Plan) {
    let key = Style::new().fg(TH.accent());
    let hint = Style::new().fg(TH.dim());
    let mut spans: Vec<Span> = Vec::new();
    let hints: &[(&str, &str)] = if plan.compact {
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
            ("esc", "back"),
            ("f", "zoom"),
        ]
    };
    for (k, v) in hints {
        spans.push(Span::styled(format!(" {k}"), key));
        spans.push(Span::styled(format!(" {v} "), hint));
    }
    let cluster = " ● service  container 1.2.0  ⠋ ";
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let cluster_len = cluster.chars().count();
    if (plan.bottom.width as usize) > used + cluster_len {
        let pad = (plan.bottom.width as usize).saturating_sub(used + cluster_len);
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled("● ", Style::new().fg(TH.accent())));
        spans.push(Span::styled("service  container 1.2.0  ", hint));
        spans.push(Span::styled("⠋", Style::new().fg(TH.dim())));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::new().bg(TH.bar())),
        plan.bottom,
    );
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let w = 72.min(area.width.saturating_sub(2));
    let h = 18.min(area.height.saturating_sub(2));
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let r = Rect::new(x, y, w, h);
    frame.render_widget(Clear, r);
    let block = pane_block(
        "react to this prototype",
        true,
        Some(Line::from(Span::styled(
            " esc closes ",
            Style::new().fg(TH.dim()),
        ))),
    );
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  1. Breakpoint",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from(
            "     F1 vs F2 vs a live resize through ~70–90. Is 80 where the rail should climb?",
        ),
        Line::from(Span::styled(
            "  2. Inactive collapse",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from("     Tight = 1-row title+count. Roomy = shrink-to-fit names. Enough?"),
        Line::from(Span::styled(
            "  3. Width cap",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from(
            "     F2/F3: spare columns go to logs (detail title shows col count). 36 greedy?",
        ),
        Line::from(Span::styled(
            "  4. 1/2/3 as expand",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from("     Panels stay; the active one grows. Filter + per-panel selection memory."),
        Line::from(Span::styled(
            "  5. 55×20 floor",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from("     Dropped: table headers, Logs/Inspect tab row, a header line. What else?"),
        Line::from(""),
        Line::from(Span::styled(
            "  F1 55×20   F2 100×30   F3 200×50   F4 live terminal",
            Style::new().fg(TH.yellow()),
        )),
    ];
    frame.render_widget(Paragraph::new(text).block(block), r);
}

// ── input ─────────────────────────────────────────────────────────────────

fn handle(state: &mut State, key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
        return false;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }
    if state.help {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')
        ) {
            state.help = false;
        }
        return false;
    }
    if state.filter_input {
        match key.code {
            KeyCode::Esc => {
                state.filter_input = false;
                state.filter.clear();
            }
            KeyCode::Enter => state.filter_input = false,
            KeyCode::Backspace => {
                state.filter.pop();
            }
            KeyCode::Char(c) => state.filter.push(c),
            _ => {}
        }
        return false;
    }
    match key.code {
        KeyCode::Char('q') => return true,
        KeyCode::Char('?') => state.help = true,
        KeyCode::F(1) => state.preset = Some(PRESET_FLOOR),
        KeyCode::F(2) => state.preset = Some(PRESET_MED),
        KeyCode::F(3) => state.preset = Some(PRESET_WIDE),
        KeyCode::F(4) => state.preset = None,
        KeyCode::Char('1') => state.pane = Pane::Containers,
        KeyCode::Char('2') => state.pane = Pane::Images,
        KeyCode::Char('3') => state.pane = Pane::Volumes,
        KeyCode::Tab => state.pane = state.pane.next(),
        KeyCode::Char('f') => state.zoom = !state.zoom,
        KeyCode::Char('/') if state.focus == Focus::List => state.filter_input = true,
        KeyCode::Enter if state.focus == Focus::List => state.focus = Focus::Detail,
        KeyCode::Esc => {
            if state.zoom {
                state.zoom = false;
            } else if !state.filter.is_empty() {
                state.filter.clear();
            } else {
                state.focus = Focus::List;
            }
        }
        KeyCode::Char('l') => state.tab = DetailTab::Logs,
        KeyCode::Char('i') => state.tab = DetailTab::Inspect,
        KeyCode::Char('j') | KeyCode::Down => {
            if state.focus == Focus::List {
                state.move_sel(1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if state.focus == Focus::List {
                state.move_sel(-1);
            }
        }
        _ => {}
    }
    false
}

// ── dump ──────────────────────────────────────────────────────────────────

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

fn dump_size(state: &State, w: u16, h: u16) -> (String, String) {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal
        .draw(|f| {
            let mut s = seed();
            s.pane = state.pane;
            s.focus = state.focus;
            s.tab = state.tab;
            s.zoom = state.zoom;
            s.filter = state.filter.clone();
            s.selected = state.selected;
            s.preset = None;
            draw(f, &s, None);
        })
        .unwrap();
    let backend = terminal.backend();
    let plan = {
        let mut s = seed();
        s.pane = state.pane;
        Plan::compute(Rect::new(0, 0, w, h), &s)
    };
    (plan.summary(state), buffer_to_string(backend.buffer()))
}

fn dump_all() -> io::Result<()> {
    let dir = Path::new("dumps");
    std::fs::create_dir_all(dir)?;
    let state = seed();
    let mut index = String::from("# unified rail ladder — static dumps\n\n");
    for (name, (w, h)) in [
        ("55x20", PRESET_FLOOR),
        ("100x30", PRESET_MED),
        ("200x50", PRESET_WIDE),
    ] {
        let (summary, frame) = dump_size(&state, w, h);
        let body = format!("# {w}×{h}\n\n{summary}\n\n```\n{frame}```\n");
        std::fs::write(dir.join(format!("{name}.txt")), &body)?;
        index.push_str(&format!("## {w}×{h}\n\n{summary}\n\n```\n{frame}```\n\n"));
        eprintln!("{w}×{h}: {summary}");
    }
    let mut images = seed();
    images.pane = Pane::Images;
    let (summary, frame) = dump_size(&images, PRESET_FLOOR.0, PRESET_FLOOR.1);
    std::fs::write(
        dir.join("55x20-images.txt"),
        format!("# 55×20 images expanded\n\n{summary}\n\n```\n{frame}```\n"),
    )?;
    index.push_str(&format!(
        "## 55×20 images expanded (`2`)\n\n{summary}\n\n```\n{frame}```\n\n"
    ));
    eprintln!("55×20 images: {summary}");
    std::fs::write(dir.join("INDEX.md"), index)?;
    eprintln!("wrote dumps/ to {}", dir.canonicalize()?.display());
    Ok(())
}

// ── main ──────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--dump") {
        return dump_all();
    }

    let mut state = seed();
    if let Some(arg) = args.iter().find(|a| a.starts_with("--size=")) {
        state.preset = match arg.split('=').nth(1).unwrap_or("") {
            "55x20" | "floor" => Some(PRESET_FLOOR),
            "100x30" | "med" => Some(PRESET_MED),
            "200x50" | "wide" => Some(PRESET_WIDE),
            _ => None,
        };
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let result = (|| -> io::Result<()> {
        loop {
            terminal.draw(|f| draw(f, &state, state.preset))?;
            if event::poll(Duration::from_millis(200))? {
                match event::read()? {
                    Event::Key(k) => {
                        if handle(&mut state, k) {
                            break;
                        }
                    }
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
        }
        Ok(())
    })();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    result
}

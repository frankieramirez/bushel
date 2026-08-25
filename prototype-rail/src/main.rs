//! PROTOTYPE — throwaway. Do not merge.
//!
//! Question: what does the persistent telemetry strip actually look like?
//! Hosted in the locked unified-rail layout so collapse at 55×20 is honest.
//!
//! Starting proposal (research #16):
//!   - 3 rows: cpu spark, mem spark, net+disk as text
//!   - Sparkline eighth-blocks, newest-first + RightToLeft, .max(100) on cpu/mem
//!   - `--ascii` / `a` swaps a custom " .:-=+*#" bar set
//!   - strip yields when the detail inner has fewer than strip_h + 4 rows

use std::collections::VecDeque;
use std::io::{self, stdout};
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, Paragraph, RenderDirection, Row, Sparkline, Table, TableState,
    Tabs,
};
use ratatui::{Frame, Terminal};

// ── starting numbers to react to ──────────────────────────────────────────

const STACK_BELOW: u16 = 80;
const RAIL_MAX: u16 = 36;
const RAIL_PCT: u16 = 45;
const TIGHT_RAIL_H: u16 = 16;

const PRESET_FLOOR: (u16, u16) = (55, 20);
const PRESET_MED: (u16, u16) = (100, 30);
const PRESET_WIDE: (u16, u16) = (200, 50);

const HISTORY: usize = 300; // 5 minutes at 1s
const STRIP_MIN_LOG: u16 = 4; // logs the strip must leave
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

#[derive(Clone, Copy)]
struct Sample {
    cpu: f64,
    mem: f64,
    rx: u64,
    tx: u64,
    r: u64,
    w: u64,
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
    telemetry: VecDeque<Sample>,
    ascii: bool,
    /// 2, 3 (default), or 4 rows.
    strip_kind: u8,
    tick: u32,
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
        telemetry: history(HISTORY as u32),
        ascii: false,
        strip_kind: 3,
        tick: HISTORY as u32,
    }
}

fn sample_at(i: u32) -> Sample {
    let t = i as f64;
    let cpu = 22.0 + 16.0 * (t / 20.0).sin() + if i % 53 == 0 { 45.0 } else { 0.0 };
    let mem = 46.0 + 8.0 * (t / 70.0).sin() + t * 0.01;
    let burst = i % 40 < 5;
    let disk = i % 70 < 3;
    Sample {
        cpu: cpu.clamp(0.0, 99.0),
        mem: mem.clamp(0.0, 96.0),
        rx: if burst { 1_400_000 } else { 12_000 },
        tx: if burst { 220_000 } else { 4_000 },
        r: if disk { 800_000 } else { 2_000 },
        w: if disk { 1_100_000 } else { 8_000 },
    }
}

fn history(end: u32) -> VecDeque<Sample> {
    let mut q = VecDeque::with_capacity(HISTORY);
    let start = end.saturating_sub(HISTORY as u32);
    for i in start..end {
        q.push_front(sample_at(i));
    }
    q
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

fn cpu_color(pct: f64) -> Color {
    if pct > 90.0 {
        TH.red()
    } else if pct > 70.0 {
        TH.yellow()
    } else {
        TH.accent()
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

    fn selected_running(&self) -> bool {
        let rows = self.visible(Pane::Containers);
        let i = *rows.get(self.selected[0]).unwrap_or(&0);
        self.containers.get(i).map(|c| c.running).unwrap_or(false)
    }

    fn latest(&self) -> Sample {
        self.telemetry.front().copied().unwrap_or(Sample {
            cpu: 0.0,
            mem: 0.0,
            rx: 0,
            tx: 0,
            r: 0,
            w: 0,
        })
    }

    fn push_sample(&mut self) {
        self.tick += 1;
        self.telemetry.push_front(sample_at(self.tick));
        while self.telemetry.len() > HISTORY {
            self.telemetry.pop_back();
        }
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
    let extra = {
        let mut bits = format!(" {log_cols} cols");
        if state.pane == Pane::Containers {
            let inner_h = area.height.saturating_sub(2);
            let want = state.strip_kind as u16;
            let shown = inner_h >= want + STRIP_MIN_LOG;
            if shown {
                bits.push_str(&format!(" · strip {want} · last {log_cols}s of 5m"));
            } else {
                bits.push_str(" · strip collapsed");
            }
        }
        bits.push(' ');
        Some(Line::from(Span::styled(bits, Style::new().fg(TH.dim()))))
    };
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

    if state.pane == Pane::Containers {
        let want = state.strip_kind as u16;
        let shown = content.height >= want + STRIP_MIN_LOG;
        if shown && content.height >= want {
            let parts =
                Layout::vertical([Constraint::Length(want), Constraint::Min(1)]).split(content);
            draw_strip(frame, state, parts[0]);
            content = parts[1];
        }
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

fn bar_set(ascii: bool) -> symbols::bar::Set<'static> {
    if ascii {
        ASCII_BARS
    } else {
        symbols::bar::NINE_LEVELS
    }
}

fn draw_spark(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: String,
    value_style: Style,
    data: &[u64],
    max: Option<u64>,
    ascii: bool,
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
            Style::new().fg(TH.dim()),
        ))),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(value, value_style))),
        parts[1],
    );
    if parts[2].width == 0 || data.is_empty() {
        return;
    }
    let mut sp = Sparkline::default()
        .data(data.iter().copied())
        .direction(RenderDirection::RightToLeft)
        .bar_set(bar_set(ascii))
        .style(Style::new().fg(TH.accent()));
    if let Some(m) = max {
        sp = sp.max(m);
    }
    frame.render_widget(sp, parts[2]);
}

fn draw_rates(frame: &mut Frame, area: Rect, s: Sample, running: bool, ascii: bool) {
    if area.width == 0 {
        return;
    }
    let up = if ascii { "^" } else { "↑" };
    let dn = if ascii { "v" } else { "↓" };
    let net = if running {
        format!(
            "net {up}{:<7} {dn}{:<7}",
            human_rate(s.rx),
            human_rate(s.tx)
        )
    } else {
        format!("net {up}-       {dn}-")
    };
    let dsk = if running {
        format!("dsk r {:<7} w {:<7}", human_rate(s.r), human_rate(s.w))
    } else {
        "dsk r -       w -".into()
    };
    let line = Line::from(vec![
        Span::styled(net, Style::new().fg(TH.text())),
        Span::styled("  ", Style::new()),
        Span::styled(dsk, Style::new().fg(TH.dim())),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_strip(frame: &mut Frame, state: &State, area: Rect) {
    let running = state.selected_running();
    let cur = state.latest();
    let cpu: Vec<u64> = state
        .telemetry
        .iter()
        .map(|s| s.cpu.round().clamp(0.0, 100.0) as u64)
        .collect();
    let mem: Vec<u64> = state
        .telemetry
        .iter()
        .map(|s| s.mem.round().clamp(0.0, 100.0) as u64)
        .collect();
    let rx: Vec<u64> = state.telemetry.iter().map(|s| s.rx).collect();
    let tx: Vec<u64> = state.telemetry.iter().map(|s| s.tx).collect();
    let (cpu_v, mem_v) = if running {
        (format!("{:>5.1}%", cur.cpu), format!("{:>5.0}%", cur.mem))
    } else {
        ("    -".into(), "    -".into())
    };
    let cpu_style = Style::new()
        .fg(if running {
            cpu_color(cur.cpu)
        } else {
            TH.dim()
        })
        .bold();
    let mem_style = Style::new().fg(if running { TH.text() } else { TH.dim() });

    match state.strip_kind {
        2 => {
            let halves =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(area);
            let left =
                Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(halves[0]);
            let right =
                Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(halves[1]);
            draw_spark(
                frame,
                left[0],
                "cpu",
                cpu_v,
                cpu_style,
                &cpu,
                Some(100),
                state.ascii,
            );
            draw_spark(
                frame,
                right[0],
                "mem",
                mem_v,
                mem_style,
                &mem,
                Some(100),
                state.ascii,
            );
            draw_rates(
                frame,
                Rect {
                    x: left[1].x,
                    y: left[1].y,
                    width: left[1].width + right[1].width,
                    height: 1,
                },
                cur,
                running,
                state.ascii,
            );
        }
        4 => {
            let rows = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(area);
            draw_spark(
                frame,
                rows[0],
                "cpu",
                cpu_v,
                cpu_style,
                &cpu,
                Some(100),
                state.ascii,
            );
            draw_spark(
                frame,
                rows[1],
                "mem",
                mem_v,
                mem_style,
                &mem,
                Some(100),
                state.ascii,
            );
            let net_max = rx
                .iter()
                .copied()
                .max()
                .unwrap_or(1)
                .max(tx.iter().copied().max().unwrap_or(1));
            let net_v = if running {
                format!("{:>5}", human_rate(cur.rx + cur.tx))
            } else {
                "    -".into()
            };
            draw_spark(
                frame,
                rows[2],
                "net",
                net_v,
                Style::new().fg(TH.text()),
                &rx,
                Some(net_max),
                state.ascii,
            );
            let dsk_max = state
                .telemetry
                .iter()
                .map(|s| s.w.max(s.r))
                .max()
                .unwrap_or(1);
            let dsk: Vec<u64> = state.telemetry.iter().map(|s| s.w).collect();
            let dsk_v = if running {
                format!("{:>5}", human_rate(cur.w))
            } else {
                "    -".into()
            };
            draw_spark(
                frame,
                rows[3],
                "dsk",
                dsk_v,
                Style::new().fg(TH.dim()),
                &dsk,
                Some(dsk_max),
                state.ascii,
            );
        }
        _ => {
            let rows = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(area);
            draw_spark(
                frame,
                rows[0],
                "cpu",
                cpu_v,
                cpu_style,
                &cpu,
                Some(100),
                state.ascii,
            );
            draw_spark(
                frame,
                rows[1],
                "mem",
                mem_v,
                mem_style,
                &mem,
                Some(100),
                state.ascii,
            );
            draw_rates(frame, rows[2], cur, running, state.ascii);
        }
    }
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
            ("s", "strip"),
            ("a", "ascii"),
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
            "  1. Height / layout",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from(
            "     `s` cycles 2 / 3 / 4 rows. Default 3: cpu spark, mem spark, net+disk text.",
        ),
        Line::from(Span::styled(
            "  2. Glyphs + ascii",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from("     Eighth-blocks. `a` swaps the ASCII ramp. Does --ascii still read?"),
        Line::from(Span::styled(
            "  3. Collapse",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from(
            "     Strip yields when detail inner < strip_h + 4. F1 + `s` to 4-row forces it.",
        ),
        Line::from(Span::styled(
            "  4. Logs vs Inspect",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from("     Same strip on both tabs (`l` / `i`). Does Inspect want it too?"),
        Line::from(Span::styled(
            "  5. Five minutes",
            Style::new().fg(TH.accent()).bold(),
        )),
        Line::from(
            "     Sparkline is one second per column, newest on the right. Detail title shows window.",
        ),
        Line::from(""),
        Line::from(Span::styled(
            "  F1 55×20  F2 100×30  F3 200×50  s layout  a ascii",
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
        KeyCode::Char('a') => state.ascii = !state.ascii,
        KeyCode::Char('s') => {
            state.strip_kind = match state.strip_kind {
                2 => 3,
                3 => 4,
                _ => 2,
            };
        }
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
            s.ascii = state.ascii;
            s.strip_kind = state.strip_kind;
            s.telemetry = state.telemetry.clone();
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
    let mut index = String::from("# telemetry strip — static dumps\n\n");
    let mut write = |name: &str, title: &str, s: &State, w: u16, h: u16| -> io::Result<()> {
        let (summary, frame) = dump_size(s, w, h);
        let body = format!("# {title}\n\n{summary}\n\n```\n{frame}```\n");
        std::fs::write(dir.join(format!("{name}.txt")), &body)?;
        index.push_str(&format!("## {title}\n\n{summary}\n\n```\n{frame}```\n\n"));
        eprintln!("{name}: {summary}");
        Ok(())
    };
    let base = seed();
    write("55x20", "55×20 logs, 3-row strip", &base, 55, 20)?;
    let mut inspect = seed();
    inspect.tab = DetailTab::Inspect;
    write(
        "55x20-inspect",
        "55×20 inspect, 3-row strip",
        &inspect,
        55,
        20,
    )?;
    write("100x30", "100×30 logs, 3-row strip", &base, 100, 30)?;
    write("200x50", "200×50 logs, 3-row strip", &base, 200, 50)?;
    let mut ascii = seed();
    ascii.ascii = true;
    write("100x30-ascii", "100×30 ascii glyphs", &ascii, 100, 30)?;
    let mut four = seed();
    four.strip_kind = 4;
    write("55x20-4row", "55×20 4-row (should collapse)", &four, 55, 20)?;
    let mut two = seed();
    two.strip_kind = 2;
    write("100x30-2row", "100×30 2-row layout", &two, 100, 30)?;
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
    if args.iter().any(|a| a == "--ascii") {
        state.ascii = true;
    }
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
    let mut last_tick = Instant::now();
    let result = (|| -> io::Result<()> {
        loop {
            if last_tick.elapsed() >= Duration::from_millis(400) {
                state.push_sample();
                last_tick = Instant::now();
            }
            terminal.draw(|f| draw(f, &state, state.preset))?;
            if event::poll(Duration::from_millis(80))? {
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

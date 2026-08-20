// PROTOTYPE — throwaway layout mock for bushel v0.1 (wayfinder ticket #7).
// Fake data only; nothing here talks to the `container` CLI.
// Run: cargo run    Flags: --no-splash --reduced-motion
// Sim keys: F1 toggle ambient effect · F2 toggle service-down · F3 external stop

use std::io::Write as _;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, Paragraph, Row, Table, TableState, Tabs, Wrap,
};
use ratatui::{DefaultTerminal, Frame};
use tachyonfx::{fx, EffectManager, Interpolation, Motion};

const BG: Color = Color::Rgb(0x0f, 0x11, 0x17);
const PANEL: Color = Color::Rgb(0x14, 0x17, 0x20);
const DIM: Color = Color::Rgb(0x5c, 0x63, 0x70);
const TEXT: Color = Color::Rgb(0xc9, 0xd1, 0xd9);
const ACCENT_A: (u8, u8, u8) = (0x7e, 0xe7, 0x87); // orchard green
const ACCENT_B: (u8, u8, u8) = (0xff, 0x7b, 0x72); // apple red
const ACCENT: Color = Color::Rgb(0x7e, 0xe7, 0x87);
const RED: Color = Color::Rgb(0xff, 0x7b, 0x72);
const YELLOW: Color = Color::Rgb(0xe3, 0xb3, 0x41);
const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Containers,
    Images,
    Volumes,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Splash,
    Main,
    ServiceDown,
}

enum Overlay {
    None,
    ActionMenu,
    Confirm { command: String, action: PendingKind },
    Help,
    MessageLog,
    PullInput { text: String },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingKind {
    Start,
    Stop,
    Kill,
    Restart,
    Delete,
    Prune,
}

struct ContainerRow {
    name: String,
    image: String,
    running: bool,
    cpu: f32,
    mem: f32,
    pending: Option<(PendingKind, Instant)>,
}

struct ImageRow {
    reference: String,
    size: &'static str,
    pending: Option<(PendingKind, Instant)>,
}

struct VolumeRow {
    name: String,
    in_use: bool,
    pending: Option<(PendingKind, Instant)>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
enum FxKey {
    #[default]
    Ambient,
}

struct App {
    screen: Screen,
    pane: Pane,
    focus: Focus,
    zoom: bool,
    detail_tab: DetailTab,
    containers: Vec<ContainerRow>,
    images: Vec<ImageRow>,
    volumes: Vec<VolumeRow>,
    sel: [usize; 3],
    filter: String,
    filter_input: bool,
    overlay: Overlay,
    banner: bool,
    messages: Vec<String>,
    status: Option<(String, Instant, bool)>, // text, when, is_error
    effects: EffectManager<FxKey>,
    reduced_motion: bool,
    ambient: bool,
    spinner: usize,
    last_poll: Instant,
    splash_start: Instant,
    splash_probe: usize,
    service_output: Vec<String>,
    service_starting: Option<Instant>,
    pull_progress: Option<(String, Instant)>,
    detail_scroll: u16,
    follow: bool,
    log_lines: Vec<String>,
    rng: u64,
    frame_times: Vec<Duration>,
    quit: bool,
}

impl App {
    fn new(reduced_motion: bool, no_splash: bool) -> Self {
        let mut app = Self {
            screen: if no_splash { Screen::Main } else { Screen::Splash },
            pane: Pane::Containers,
            focus: Focus::List,
            zoom: false,
            detail_tab: DetailTab::Logs,
            containers: fake_containers(),
            images: fake_images(),
            volumes: fake_volumes(),
            sel: [0; 3],
            filter: String::new(),
            filter_input: false,
            overlay: Overlay::None,
            banner: true,
            messages: vec!["bushel prototype started".into()],
            status: None,
            effects: EffectManager::default(),
            reduced_motion,
            ambient: true,
            spinner: 0,
            last_poll: Instant::now(),
            splash_start: Instant::now(),
            splash_probe: 0,
            service_output: Vec::new(),
            service_starting: None,
            pull_progress: None,
            detail_scroll: 0,
            follow: true,
            log_lines: Vec::new(),
            rng: 0x5eed_cafe,
            frame_times: Vec::new(),
            quit: false,
        };
        app.log_lines = fake_logs(&mut app.rng);
        app
    }

    fn rand(&mut self) -> u64 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.rng >> 33
    }

    fn fx(&mut self, effect: tachyonfx::Effect) {
        if !self.reduced_motion {
            self.effects.add_effect(effect);
        }
    }

    fn toast(&mut self, msg: impl Into<String>, error: bool) {
        let msg = msg.into();
        self.messages.push(msg.clone());
        self.status = Some((msg, Instant::now(), error));
    }

    fn visible_rows(&self) -> Vec<usize> {
        let f = self.filter.to_lowercase();
        let matches = |hay: &str| f.is_empty() || hay.to_lowercase().contains(&f);
        match self.pane {
            Pane::Containers => self
                .containers
                .iter()
                .enumerate()
                .filter(|(_, c)| matches(&format!("{} {} {}", c.name, c.image, if c.running { "running" } else { "stopped" })))
                .map(|(i, _)| i)
                .collect(),
            Pane::Images => self
                .images
                .iter()
                .enumerate()
                .filter(|(_, i)| matches(&i.reference))
                .map(|(i, _)| i)
                .collect(),
            Pane::Volumes => self
                .volumes
                .iter()
                .enumerate()
                .filter(|(_, v)| matches(&v.name))
                .map(|(i, _)| i)
                .collect(),
        }
    }

    fn pane_idx(&self) -> usize {
        match self.pane {
            Pane::Containers => 0,
            Pane::Images => 1,
            Pane::Volumes => 2,
        }
    }

    fn selected_container(&self) -> Option<&ContainerRow> {
        let rows = self.visible_rows();
        rows.get(self.sel[0]).map(|&i| &self.containers[i])
    }

    fn switch_pane(&mut self, pane: Pane, area: Rect) {
        if self.pane == pane {
            return;
        }
        self.pane = pane;
        self.focus = Focus::List;
        self.detail_scroll = 0;
        self.filter.clear();
        self.filter_input = false;
        self.fx(
            fx::sweep_in(Motion::LeftToRight, 12, 0, BG, (120, Interpolation::QuadOut))
                .with_area(area),
        );
    }
}

fn fake_containers() -> Vec<ContainerRow> {
    let data: [(&str, &str, bool); 8] = [
        ("api-gateway", "nginx:1.27", true),
        ("web-frontend", "node:22-alpine", true),
        ("postgres-main", "postgres:16", true),
        ("redis-cache", "redis:7-alpine", true),
        ("worker-emails", "ghcr.io/acme/worker:2.3", true),
        ("legacy-cron", "alpine:3.20", false),
        ("db-migrate", "flyway:10", false),
        ("scratchpad", "ubuntu:24.04", false),
    ];
    data.iter()
        .enumerate()
        .map(|(i, (n, img, run))| ContainerRow {
            name: n.to_string(),
            image: img.to_string(),
            running: *run,
            cpu: if *run { 2.0 + i as f32 * 3.7 } else { 0.0 },
            mem: if *run { 48.0 + i as f32 * 61.0 } else { 0.0 },
            pending: None,
        })
        .collect()
}

fn fake_images() -> Vec<ImageRow> {
    [
        ("alpine:3.20", "3.2 MB"),
        ("flyway:10", "212 MB"),
        ("ghcr.io/acme/worker:2.3", "89 MB"),
        ("nginx:1.27", "68 MB"),
        ("node:22-alpine", "52 MB"),
        ("postgres:16", "148 MB"),
        ("redis:7-alpine", "17 MB"),
        ("ubuntu:24.04", "29 MB"),
    ]
    .iter()
    .map(|(r, s)| ImageRow { reference: r.to_string(), size: s, pending: None })
    .collect()
}

fn fake_volumes() -> Vec<VolumeRow> {
    [
        ("caddy-data", false),
        ("pg-data", true),
        ("redis-data", true),
        ("scratch", false),
        ("worker-spool", true),
    ]
    .iter()
    .map(|(n, u)| VolumeRow { name: n.to_string(), in_use: *u, pending: None })
    .collect()
}

fn fake_logs(rng: &mut u64) -> Vec<String> {
    let paths = ["/healthz", "/api/v1/orders", "/api/v1/users/42", "/metrics", "/login", "/static/app.js"];
    let mut lines = Vec::new();
    for i in 0..48 {
        *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r = (*rng >> 33) as usize;
        let status = if r % 17 == 0 { 500 } else if r % 9 == 0 { 404 } else { 200 };
        lines.push(format!(
            "2026-08-19T09:{:02}:{:02}Z  GET {} {} {}ms",
            10 + i / 60,
            i % 60,
            paths[r % paths.len()],
            status,
            1 + r % 40
        ));
    }
    lines
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> Color {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color::Rgb(f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

fn gradient_spans(text: &str, bold: bool) -> Vec<Span<'static>> {
    let n = text.chars().count().max(1);
    text.chars()
        .enumerate()
        .map(|(i, c)| {
            let mut style = Style::new().fg(lerp(ACCENT_A, ACCENT_B, i as f32 / (n - 1).max(1) as f32));
            if bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            Span::styled(c.to_string(), style)
        })
        .collect()
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let reduced = args.iter().any(|a| a == "--reduced-motion");
    let no_splash = args.iter().any(|a| a == "--no-splash");
    let mut terminal = ratatui::init();
    let mut app = App::new(reduced, no_splash);
    if !no_splash && reduced {
        app.screen = Screen::Main;
    }
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    if let Some(avg) = app.frame_times.iter().map(|d| d.as_micros()).max() {
        eprintln!("prototype: {} frames, worst draw {avg}µs", app.frame_times.len());
    }
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut last_frame = Instant::now();
    loop {
        if app.quit {
            return Ok(());
        }
        tick(app);
        let elapsed = last_frame.elapsed();
        last_frame = Instant::now();
        let t0 = Instant::now();
        terminal.draw(|f| draw(f, app, elapsed))?;
        app.frame_times.push(t0.elapsed());

        let animating = app.effects.is_running()
            || app.screen == Screen::Splash
            || app.service_starting.is_some()
            || app.pull_progress.is_some();
        let timeout = if animating { Duration::from_millis(33) } else { Duration::from_millis(150) };
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key(app, key, terminal)?;
                }
                Event::Resize(_, _) => {
                    if app.ambient {
                        app.effects.cancel_unique_effect(FxKey::Ambient);
                        app.ambient = false; // re-enabled next draw via ensure_ambient
                        app.ambient = true;
                    }
                }
                _ => {}
            }
        }
    }
}

fn tick(app: &mut App) {
    // splash probes advance on a fake schedule; done after ~1.4s
    if app.screen == Screen::Splash {
        let ms = app.splash_start.elapsed().as_millis();
        app.splash_probe = (ms / 450) as usize;
        if ms > 1400 {
            enter_main(app);
        }
    }

    // fake service start stream
    if let Some(started) = app.service_starting {
        let steps = [
            "installing kernel … ok (cached)",
            "starting container-apiserver …",
            "container-apiserver listening",
            "service is up",
        ];
        let n = (started.elapsed().as_millis() / 500) as usize;
        while app.service_output.len() < n.min(steps.len()) {
            let line = steps[app.service_output.len()];
            app.service_output.push(line.to_string());
        }
        if n > steps.len() {
            app.service_starting = None;
            app.screen = Screen::Main;
            app.toast("service started", false);
            app.fx(fx::coalesce((150, Interpolation::QuadOut)));
        }
    }

    // fake pull progress finishes after 3s
    if let Some((ref reference, started)) = app.pull_progress {
        if started.elapsed() > Duration::from_secs(3) {
            let reference = reference.clone();
            app.images.push(ImageRow { reference: reference.clone(), size: "41 MB", pending: None });
            app.images.sort_by(|a, b| a.reference.cmp(&b.reference));
            app.pull_progress = None;
            app.toast(format!("pulled {reference}"), false);
        }
    }

    // poll tick: 1s cadence — jitter stats, resolve pending, grow logs
    if app.last_poll.elapsed() >= Duration::from_secs(1) {
        app.last_poll = Instant::now();
        app.spinner = (app.spinner + 1) % SPINNER.len();
        for i in 0..app.containers.len() {
            if app.containers[i].running {
                let r = (app.rand() % 100) as f32 / 100.0;
                app.containers[i].cpu = (app.containers[i].cpu + r * 2.0 - 1.0).clamp(0.2, 97.0);
                let r2 = (app.rand() % 100) as f32 / 100.0;
                app.containers[i].mem = (app.containers[i].mem + r2 * 8.0 - 4.0).clamp(16.0, 900.0);
            }
        }
        if app.follow && app.detail_tab == DetailTab::Logs {
            let extra = fake_logs(&mut app.rng);
            let pick = (app.rand() % extra.len() as u64) as usize;
            app.log_lines.push(extra[pick].clone());
        }
        resolve_pending(app);
    }

    // expire toast
    if let Some((_, when, _)) = app.status {
        if when.elapsed() > Duration::from_secs(3) {
            app.status = None;
        }
    }
}

fn enter_main(app: &mut App) {
    app.screen = Screen::Main;
    app.fx(fx::coalesce((150, Interpolation::QuadOut)));
}

fn resolve_pending(app: &mut App) {
    let done = |p: &Option<(PendingKind, Instant)>| {
        matches!(p, Some((_, t)) if t.elapsed() > Duration::from_millis(1800))
    };
    let mut msgs: Vec<String> = Vec::new();
    app.containers.retain_mut(|c| {
        if done(&c.pending) {
            let (kind, _) = c.pending.take().unwrap();
            match kind {
                PendingKind::Delete => {
                    msgs.push(format!("deleted {}", c.name));
                    return false;
                }
                PendingKind::Stop | PendingKind::Kill => {
                    c.running = false;
                    c.cpu = 0.0;
                    c.mem = 0.0;
                    msgs.push(format!("stopped {}", c.name));
                }
                PendingKind::Start | PendingKind::Restart => {
                    c.running = true;
                    c.cpu = 1.0;
                    c.mem = 32.0;
                    msgs.push(format!("started {}", c.name));
                }
                PendingKind::Prune => {}
            }
        }
        true
    });
    app.images.retain_mut(|i| {
        if done(&i.pending) {
            msgs.push(format!("deleted {}", i.reference));
            return false;
        }
        true
    });
    app.volumes.retain_mut(|v| {
        if done(&v.pending) {
            msgs.push(format!("deleted {}", v.name));
            return false;
        }
        true
    });
    for m in msgs {
        app.toast(m, false);
    }
    let counts = [app.visible_rows().len()];
    let idx = app.pane_idx();
    if app.sel[idx] >= counts[0] && counts[0] > 0 {
        app.sel[idx] = counts[0] - 1;
    }
}

fn handle_key(app: &mut App, key: KeyEvent, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.quit = true;
        return Ok(());
    }

    // splash: any key skips
    if app.screen == Screen::Splash {
        enter_main(app);
        return Ok(());
    }

    // service-down takeover
    if app.screen == Screen::ServiceDown {
        match key.code {
            KeyCode::Char('s') if app.service_starting.is_none() => {
                app.service_starting = Some(Instant::now());
                app.service_output.clear();
                app.service_output.push("$ container system start --enable-kernel-install".into());
            }
            KeyCode::Char('q') => app.quit = true,
            KeyCode::F(2) => {
                app.screen = Screen::Main;
                app.fx(fx::coalesce((150, Interpolation::QuadOut)));
            }
            _ => {}
        }
        return Ok(());
    }

    // overlays capture input first
    match &mut app.overlay {
        Overlay::Confirm { command, action } => {
            match key.code {
                KeyCode::Char('y') => {
                    let action = *action;
                    let command = command.clone();
                    app.overlay = Overlay::None;
                    apply_action(app, action, &command);
                }
                KeyCode::Esc | KeyCode::Char('n') => app.overlay = Overlay::None,
                _ => {}
            }
            return Ok(());
        }
        Overlay::Help | Overlay::MessageLog => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Char('m')) {
                app.overlay = Overlay::None;
            }
            return Ok(());
        }
        Overlay::ActionMenu => {
            match key.code {
                KeyCode::Esc | KeyCode::Char(' ') => app.overlay = Overlay::None,
                KeyCode::Char(c) => {
                    app.overlay = Overlay::None;
                    action_key(app, c, terminal)?;
                }
                _ => {}
            }
            return Ok(());
        }
        Overlay::PullInput { text } => {
            match key.code {
                KeyCode::Esc => app.overlay = Overlay::None,
                KeyCode::Enter => {
                    let reference = text.clone();
                    app.overlay = Overlay::None;
                    if !reference.is_empty() {
                        app.pull_progress = Some((reference.clone(), Instant::now()));
                        app.toast(format!("pulling {reference} …"), false);
                    }
                }
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char(c) => text.push(c),
                _ => {}
            }
            return Ok(());
        }
        Overlay::None => {}
    }

    // filter input mode
    if app.filter_input {
        match key.code {
            KeyCode::Esc => {
                app.filter.clear();
                app.filter_input = false;
            }
            KeyCode::Enter => app.filter_input = false,
            KeyCode::Backspace => {
                app.filter.pop();
            }
            KeyCode::Char(c) => {
                app.filter.push(c);
                app.sel[app.pane_idx()] = 0;
            }
            _ => {}
        }
        return Ok(());
    }

    // global keys
    let body = Rect::new(0, 3, 200, 50); // effect area approximation; real area set at draw
    match key.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Char('1') => app.switch_pane(Pane::Containers, body),
        KeyCode::Char('2') => app.switch_pane(Pane::Images, body),
        KeyCode::Char('3') => app.switch_pane(Pane::Volumes, body),
        KeyCode::Tab => {
            let next = match app.pane {
                Pane::Containers => Pane::Images,
                Pane::Images => Pane::Volumes,
                Pane::Volumes => Pane::Containers,
            };
            app.switch_pane(next, body);
        }
        KeyCode::Char('?') => app.overlay = Overlay::Help,
        KeyCode::Char('m') => app.overlay = Overlay::MessageLog,
        KeyCode::Char('b') => app.banner = false,
        KeyCode::F(1) => {
            app.ambient = !app.ambient;
            if !app.ambient {
                app.effects.cancel_unique_effect(FxKey::Ambient);
            }
            let state = if app.ambient { "on" } else { "off" };
            app.toast(format!("ambient effect {state}"), false);
        }
        KeyCode::F(2) => {
            app.screen = Screen::ServiceDown;
            app.service_starting = None;
            app.service_output.clear();
        }
        KeyCode::F(3) => {
            if let Some(c) = app.containers.iter_mut().find(|c| c.running && c.pending.is_none()) {
                c.running = false;
                c.cpu = 0.0;
                c.mem = 0.0;
                let name = c.name.clone();
                app.toast(format!("{name} stopped externally"), true);
            }
        }
        KeyCode::Char('f') => app.zoom = !app.zoom,
        KeyCode::Char('/') if app.focus == Focus::List => {
            app.filter_input = true;
        }
        KeyCode::Enter if app.focus == Focus::List => {
            app.focus = Focus::Detail;
            app.fx(fx::fade_from(DIM, BG, (120, Interpolation::QuadOut)));
        }
        KeyCode::Esc => {
            if app.focus == Focus::Detail {
                app.focus = Focus::List;
            } else if !app.filter.is_empty() {
                app.filter.clear();
            } else if app.zoom {
                app.zoom = false;
            }
        }
        KeyCode::Char(' ') if app.focus == Focus::List => {
            app.overlay = Overlay::ActionMenu;
            app.fx(fx::slide_in(Motion::DownToUp, 6, 0, BG, (120, Interpolation::QuadOut)));
        }
        KeyCode::PageDown => app.detail_scroll = app.detail_scroll.saturating_add(10),
        KeyCode::PageUp => app.detail_scroll = app.detail_scroll.saturating_sub(10),
        KeyCode::Char('l') if app.pane == Pane::Containers => {
            app.detail_tab = DetailTab::Logs;
            app.detail_scroll = 0;
        }
        KeyCode::Char('i') if app.pane == Pane::Containers => {
            app.detail_tab = DetailTab::Inspect;
            app.detail_scroll = 0;
        }
        _ => match app.focus {
            Focus::List => list_key(app, key, terminal)?,
            Focus::Detail => detail_key(app, key),
        },
    }
    Ok(())
}

fn list_key(app: &mut App, key: KeyEvent, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let count = app.visible_rows().len();
    let idx = app.pane_idx();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if count > 0 {
                app.sel[idx] = (app.sel[idx] + 1).min(count - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => app.sel[idx] = app.sel[idx].saturating_sub(1),
        KeyCode::Char('g') => app.sel[idx] = 0,
        KeyCode::Char('G') => app.sel[idx] = count.saturating_sub(1),
        KeyCode::Char(c) => action_key(app, c, terminal)?,
        _ => {}
    }
    Ok(())
}

fn detail_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => app.detail_scroll = app.detail_scroll.saturating_add(1),
        KeyCode::Char('k') | KeyCode::Up => app.detail_scroll = app.detail_scroll.saturating_sub(1),
        KeyCode::Char('g') => app.detail_scroll = 0,
        KeyCode::Char('G') => app.detail_scroll = u16::MAX / 2,
        KeyCode::Char('F') => app.follow = !app.follow,
        _ => {}
    }
}

fn action_key(app: &mut App, c: char, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let rows = app.visible_rows();
    let Some(&i) = rows.get(app.sel[app.pane_idx()]) else { return Ok(()) };
    match (app.pane, c) {
        (Pane::Containers, 's') => {
            let running = app.containers[i].running;
            let name = app.containers[i].name.clone();
            let (kind, verb) = if running { (PendingKind::Stop, "stopping") } else { (PendingKind::Start, "starting") };
            set_pending(app, kind);
            app.toast(format!("{verb} {name} …"), false);
        }
        (Pane::Containers, 'r') => {
            set_pending(app, PendingKind::Restart);
            app.toast("restarting (stop, then start) …", false);
        }
        (Pane::Containers, 'K') => {
            let name = app.containers[i].name.clone();
            confirm(app, format!("container kill {name}"), PendingKind::Kill);
        }
        (Pane::Containers, 'd') => {
            let name = app.containers[i].name.clone();
            confirm(app, format!("container delete {name}"), PendingKind::Delete);
        }
        (Pane::Containers, 'P') => confirm(app, "container delete --all".into(), PendingKind::Prune),
        (Pane::Containers, 'e') => {
            // real suspend/restore demo: drop the TUI, hand the terminal to a shell
            ratatui::restore();
            println!("\n— bushel prototype: exec demo — type `exit` to return —\n");
            std::io::stdout().flush()?;
            let _ = std::process::Command::new("/bin/sh").status();
            *terminal = ratatui::init();
            terminal.clear()?;
            app.fx(fx::coalesce((150, Interpolation::QuadOut)));
        }
        (Pane::Images, 'd') => {
            let r = app.images[i].reference.clone();
            confirm(app, format!("container image delete {r}"), PendingKind::Delete);
        }
        (Pane::Images, 'P') => confirm(app, "container image prune".into(), PendingKind::Prune),
        (Pane::Images, 'u') => app.overlay = Overlay::PullInput { text: String::new() },
        (Pane::Volumes, 'd') => {
            let v = &app.volumes[i];
            if v.in_use {
                let name = v.name.clone();
                app.toast(format!("cannot delete {name}: volume is in use — see message log ([m])"), true);
                app.messages.push(format!(
                    "container volume delete {name}\nError: volume \"{name}\" is in use by container \"postgres-main\" (full stderr preserved here)"
                ));
            } else {
                let name = v.name.clone();
                confirm(app, format!("container volume delete {name}"), PendingKind::Delete);
            }
        }
        (Pane::Volumes, 'P') => confirm(app, "container volume prune".into(), PendingKind::Prune),
        _ => {}
    }
    Ok(())
}

fn confirm(app: &mut App, command: String, action: PendingKind) {
    app.overlay = Overlay::Confirm { command, action };
    app.fx(fx::fade_from(DIM, BG, (100, Interpolation::QuadOut)));
}

fn set_pending(app: &mut App, kind: PendingKind) {
    let rows = app.visible_rows();
    let Some(&i) = rows.get(app.sel[app.pane_idx()]) else { return };
    match app.pane {
        Pane::Containers => app.containers[i].pending = Some((kind, Instant::now())),
        Pane::Images => app.images[i].pending = Some((kind, Instant::now())),
        Pane::Volumes => app.volumes[i].pending = Some((kind, Instant::now())),
    }
}

fn apply_action(app: &mut App, action: PendingKind, command: &str) {
    match action {
        PendingKind::Prune => {
            app.toast(format!("ran: {command}"), false);
            match app.pane {
                Pane::Containers => app.containers.retain(|c| c.running),
                Pane::Images => {
                    let used: Vec<String> = app.containers.iter().map(|c| c.image.clone()).collect();
                    app.images.retain(|i| used.contains(&i.reference));
                }
                Pane::Volumes => app.volumes.retain(|v| v.in_use),
            }
        }
        _ => set_pending(app, action),
    }
}

// ---------------------------------------------------------------- rendering

fn draw(frame: &mut Frame, app: &mut App, elapsed: Duration) {
    let area = frame.area();
    frame.render_widget(Block::new().style(Style::new().bg(BG).fg(TEXT)), area);

    match app.screen {
        Screen::Splash => draw_splash(frame, app),
        Screen::ServiceDown => draw_service_down(frame, app),
        Screen::Main => draw_main(frame, app),
    }

    app.effects.process_effects(elapsed.into(), frame.buffer_mut(), area);
}

fn draw_splash(frame: &mut Frame, app: &App) {
    let art = [
        r"   ,--./,-.                                     ",
        r"  / #   ,--\        _               _          _ ",
        r" |     |   |       | |__  _   _ ___| |__   ___| |",
        r" |     `---|       | '_ \| | | / __| '_ \ / _ \ |",
        r"  \        /       | |_) | |_| \__ \ | | |  __/ |",
        r"   `._,._,'        |_.__/ \__,_|___/_| |_|\___|_|",
    ];
    let probes = ["probing container system status …", "listing containers …", "listing images …"];
    let area = centered(frame.area(), 52, (art.len() + probes.len() + 3) as u16);
    let mut lines: Vec<Line> = art
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let t = i as f32 / (art.len() - 1) as f32;
            Line::from(Span::styled(*l, Style::new().fg(lerp(ACCENT_A, ACCENT_B, t))))
        })
        .collect();
    lines.push(Line::raw(""));
    for (i, p) in probes.iter().enumerate() {
        let done = app.splash_probe > i;
        let mark = if done { "✓" } else { SPINNER[app.spinner] };
        let style = if done { Style::new().fg(ACCENT) } else { Style::new().fg(DIM) };
        lines.push(Line::from(Span::styled(format!("  {mark} {p}"), style)));
    }
    lines.push(Line::from(Span::styled("  any key skips", Style::new().fg(DIM).italic())));
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_service_down(frame: &mut Frame, app: &App) {
    let area = centered(frame.area(), 64, 14);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(RED))
        .title(Line::from(gradient_spans(" bushel ", true)));
    let mut lines = vec![
        Line::raw(""),
        Line::from(Span::styled("  the container system service is not running", Style::new().fg(RED).bold())),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  [s]", Style::new().fg(ACCENT).bold()),
            Span::raw(" run "),
            Span::styled("container system start --enable-kernel-install", Style::new().fg(YELLOW)),
        ]),
        Line::from(vec![Span::styled("  [q]", Style::new().fg(ACCENT).bold()), Span::raw(" quit    (F2 = simulate recovery)")]),
        Line::raw(""),
    ];
    for l in &app.service_output {
        lines.push(Line::from(Span::styled(format!("  {l}"), Style::new().fg(DIM))));
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_main(frame: &mut Frame, app: &mut App) {
    let mut constraints = vec![Constraint::Length(2)];
    if app.banner {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(3));
    constraints.push(Constraint::Length(1));
    let chunks = Layout::vertical(constraints).split(frame.area());
    let header = chunks[0];
    let (banner_area, body, bottom) = if app.banner {
        (Some(chunks[1]), chunks[2], chunks[3])
    } else {
        (None, chunks[1], chunks[2])
    };

    draw_header(frame, app, header);
    if let Some(b) = banner_area {
        let line = Line::from(vec![
            Span::styled(" ⚠ container CLI 1.3.0 detected — bushel is tested against 1.2.x ", Style::new().fg(BG).bg(YELLOW)),
            Span::styled("  [b] dismiss", Style::new().fg(YELLOW)),
        ]);
        frame.render_widget(Paragraph::new(line), b);
    }

    // split or zoom
    if app.zoom {
        match app.focus {
            Focus::List => draw_list(frame, app, body),
            Focus::Detail => draw_detail(frame, app, body),
        }
    } else {
        let halves = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(body);
        draw_list(frame, app, halves[0]);
        draw_detail(frame, app, halves[1]);
    }

    draw_bottom_bar(frame, app, bottom);

    // overlays
    match &app.overlay {
        Overlay::ActionMenu => draw_action_menu(frame, app, body, bottom),
        Overlay::Confirm { command, action } => draw_confirm(frame, command, *action),
        Overlay::Help => draw_help(frame),
        Overlay::MessageLog => draw_message_log(frame, app),
        Overlay::PullInput { text } => draw_pull_input(frame, text),
        Overlay::None => {}
    }

    ensure_ambient(app, header);
}

fn ensure_ambient(app: &mut App, header: Rect) {
    if app.ambient && !app.reduced_motion && !app.effects.is_running() {
        let effect = fx::repeating(fx::ping_pong(fx::hsl_shift_fg(
            [50.0, 10.0, 6.0],
            (2800, Interpolation::SineInOut),
        )))
        .with_area(Rect { height: 1, width: 10, ..header });
        let effect = app.effects.unique(FxKey::Ambient, effect);
        app.effects.add_effect(effect);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = gradient_spans(" bushel ", true);
    spans.push(Span::raw("  "));
    for (i, (label, pane)) in [("1 containers", Pane::Containers), ("2 images", Pane::Images), ("3 volumes", Pane::Volumes)]
        .iter()
        .enumerate()
    {
        if i > 0 {
            spans.push(Span::styled("  ·  ", Style::new().fg(DIM)));
        }
        let style = if app.pane == *pane {
            Style::new().fg(ACCENT).bold().underlined()
        } else {
            Style::new().fg(DIM)
        };
        spans.push(Span::styled(*label, style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn pane_block(title: &str, focused: bool, extra: Option<Line<'static>>) -> Block<'static> {
    let border = if focused { Style::new().fg(ACCENT) } else { Style::new().fg(DIM) };
    let mut b = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(format!(" {title} "), if focused { Style::new().fg(ACCENT).bold() } else { Style::new().fg(TEXT) }))
        .style(Style::new().bg(PANEL));
    if let Some(l) = extra {
        b = b.title_bottom(l);
    }
    b
}

fn pending_span(p: &Option<(PendingKind, Instant)>, spinner: usize) -> Option<Span<'static>> {
    p.as_ref().map(|_| Span::styled(format!("{} ", SPINNER[spinner]), Style::new().fg(YELLOW)))
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::List;
    let filter_line = if app.filter_input || !app.filter.is_empty() {
        let cursor = if app.filter_input { "▏" } else { "" };
        Some(Line::from(vec![
            Span::styled(" /", Style::new().fg(ACCENT).bold()),
            Span::styled(format!("{}{cursor} ", app.filter), Style::new().fg(TEXT)),
        ]))
    } else {
        None
    };
    let title = match app.pane {
        Pane::Containers => "containers",
        Pane::Images => "images",
        Pane::Volumes => "volumes",
    };
    let block = pane_block(title, focused, filter_line);

    let rows_idx = app.visible_rows();
    let sel = app.sel[app.pane_idx()].min(rows_idx.len().saturating_sub(1));
    let highlight = Style::new().bg(Color::Rgb(0x24, 0x2b, 0x3a)).fg(TEXT).bold();

    let (header, rows, widths): (Row, Vec<Row>, Vec<Constraint>) = match app.pane {
        Pane::Containers => {
            let mut sorted = rows_idx.clone();
            sorted.sort_by_key(|&i| (!app.containers[i].running, app.containers[i].name.clone()));
            let rows = sorted
                .iter()
                .map(|&i| {
                    let c = &app.containers[i];
                    let dot = if c.running {
                        Span::styled("● ", Style::new().fg(ACCENT))
                    } else {
                        Span::styled("○ ", Style::new().fg(DIM))
                    };
                    let mut name_spans = vec![dot];
                    if let Some(s) = pending_span(&c.pending, app.spinner) {
                        name_spans.push(s);
                    }
                    name_spans.push(Span::raw(c.name.clone()));
                    let style = if c.running { Style::new().fg(TEXT) } else { Style::new().fg(DIM) };
                    Row::new(vec![
                        Cell::from(Line::from(name_spans)),
                        Cell::from(if c.running { format!("{:>4.1}%", c.cpu) } else { "-".into() }),
                        Cell::from(if c.running { format!("{:>5.0}M", c.mem) } else { "-".into() }),
                        Cell::from(c.image.clone()),
                    ])
                    .style(style)
                })
                .collect();
            (
                Row::new(vec!["name", "cpu", "mem", "image"]).style(Style::new().fg(DIM).bold()),
                rows,
                vec![Constraint::Min(18), Constraint::Length(6), Constraint::Length(7), Constraint::Min(12)],
            )
        }
        Pane::Images => {
            let rows = rows_idx
                .iter()
                .map(|&i| {
                    let im = &app.images[i];
                    let mut spans = Vec::new();
                    if let Some(s) = pending_span(&im.pending, app.spinner) {
                        spans.push(s);
                    }
                    spans.push(Span::raw(im.reference.clone()));
                    Row::new(vec![Cell::from(Line::from(spans)), Cell::from(im.size)])
                })
                .collect();
            (
                Row::new(vec!["reference", "size"]).style(Style::new().fg(DIM).bold()),
                rows,
                vec![Constraint::Min(24), Constraint::Length(8)],
            )
        }
        Pane::Volumes => {
            let rows = rows_idx
                .iter()
                .map(|&i| {
                    let v = &app.volumes[i];
                    let mut spans = Vec::new();
                    if let Some(s) = pending_span(&v.pending, app.spinner) {
                        spans.push(s);
                    }
                    spans.push(Span::raw(v.name.clone()));
                    let badge = if v.in_use {
                        Span::styled("in use", Style::new().fg(YELLOW))
                    } else {
                        Span::styled("-", Style::new().fg(DIM))
                    };
                    Row::new(vec![Cell::from(Line::from(spans)), Cell::from(Line::from(badge))])
                })
                .collect();
            (
                Row::new(vec!["name", ""]).style(Style::new().fg(DIM).bold()),
                rows,
                vec![Constraint::Min(20), Constraint::Length(8)],
            )
        }
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(highlight)
        .highlight_symbol("");
    let mut state = TableState::default();
    state.select(Some(sel));
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Detail;
    let block = pane_block("detail", focused, None);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // pull progress hijacks the detail pane while active (never a modal)
    if let Some((reference, started)) = &app.pull_progress {
        let pct = (started.elapsed().as_millis() as f32 / 3000.0 * 100.0).min(100.0);
        let bar_w = (inner.width.saturating_sub(4)) as usize;
        let filled = (bar_w as f32 * pct / 100.0) as usize;
        let lines = vec![
            Line::raw(""),
            Line::from(Span::styled(format!("  pulling {reference}"), Style::new().fg(TEXT).bold())),
            Line::raw(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("█".repeat(filled), Style::new().fg(ACCENT)),
                Span::styled("░".repeat(bar_w - filled), Style::new().fg(DIM)),
            ]),
            Line::from(Span::styled(format!("  {pct:>3.0}%"), Style::new().fg(DIM))),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let mut content_area = inner;
    if app.pane == Pane::Containers {
        let tabs_area = Rect { height: 1, ..inner };
        content_area = Rect { y: inner.y + 1, height: inner.height.saturating_sub(1), ..inner };
        let idx = if app.detail_tab == DetailTab::Logs { 0 } else { 1 };
        let tabs = Tabs::new(vec![" Logs [l] ", " Inspect [i] "])
            .select(idx)
            .style(Style::new().fg(DIM))
            .highlight_style(Style::new().fg(ACCENT).bold().underlined());
        frame.render_widget(tabs, tabs_area);
    }

    let lines: Vec<Line> = match app.pane {
        Pane::Containers => match app.detail_tab {
            DetailTab::Logs => {
                let mut l: Vec<Line> = app
                    .log_lines
                    .iter()
                    .map(|s| {
                        let style = if s.contains(" 500 ") {
                            Style::new().fg(RED)
                        } else if s.contains(" 404 ") {
                            Style::new().fg(YELLOW)
                        } else {
                            Style::new().fg(TEXT)
                        };
                        Line::from(Span::styled(s.clone(), style))
                    })
                    .collect();
                let follow = if app.follow {
                    Span::styled("── following (F to pause) ──", Style::new().fg(ACCENT))
                } else {
                    Span::styled("── paused (F to follow) ──", Style::new().fg(DIM))
                };
                l.push(Line::from(follow));
                l
            }
            DetailTab::Inspect => {
                let c = app.selected_container();
                inspect_json(c)
            }
        },
        Pane::Images => {
            let rows = app.visible_rows();
            let r = rows.get(app.sel[1]).map(|&i| app.images[i].reference.clone()).unwrap_or_default();
            vec![
                Line::raw("{"),
                Line::raw(format!("  \"reference\": \"{r}\",")),
                Line::raw("  \"digest\": \"sha256:9d3c…41af\","),
                Line::raw("  \"os\": \"linux\", \"arch\": \"arm64\","),
                Line::raw("  \"created\": \"2026-07-30T11:02:44Z\""),
                Line::raw("}"),
            ]
        }
        Pane::Volumes => {
            let rows = app.visible_rows();
            let (name, in_use) = rows
                .get(app.sel[2])
                .map(|&i| (app.volumes[i].name.clone(), app.volumes[i].in_use))
                .unwrap_or_default();
            vec![
                Line::raw("{"),
                Line::raw(format!("  \"name\": \"{name}\",")),
                Line::raw("  \"driver\": \"local\","),
                Line::raw(format!("  \"inUseBy\": {},", if in_use { "[\"postgres-main\"]" } else { "[]" })),
                Line::raw("  \"created\": \"2026-06-12T08:15:00Z\""),
                Line::raw("}"),
            ]
        }
    };

    // logs stick to bottom while following; otherwise manual scroll
    let total = lines.len() as u16;
    let h = content_area.height;
    let scroll = if app.pane == Pane::Containers && app.detail_tab == DetailTab::Logs && app.follow {
        total.saturating_sub(h)
    } else {
        app.detail_scroll.min(total.saturating_sub(1))
    };
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), content_area);
}

fn inspect_json(c: Option<&ContainerRow>) -> Vec<Line<'static>> {
    let Some(c) = c else { return vec![Line::raw("no selection")] };
    let key = Style::new().fg(ACCENT);
    let val = Style::new().fg(TEXT);
    let raw = |s: &str| Line::from(Span::styled(s.to_string(), Style::new().fg(DIM)));
    vec![
        raw("{"),
        Line::from(vec![Span::styled("  \"configuration\": ", key), Span::styled("{", val)]),
        Line::from(vec![Span::styled("    \"id\": ", key), Span::styled(format!("\"{}\",", c.name), val)]),
        Line::from(vec![Span::styled("    \"image\": ", key), Span::styled(format!("\"{}\",", c.image), val)]),
        Line::from(vec![Span::styled("    \"resources\": ", key), Span::styled("{ \"cpus\": 4, \"memoryInBytes\": 1073741824 },", val)]),
        Line::from(vec![Span::styled("    \"networks\": ", key), Span::styled("[\"default\"]", val)]),
        raw("  },"),
        Line::from(vec![
            Span::styled("  \"status\": ", key),
            Span::styled(format!("\"{}\",", if c.running { "running" } else { "stopped" }), val),
        ]),
        Line::from(vec![Span::styled("  \"networks\": ", key), Span::styled("[{ \"address\": \"192.168.64.7/24\" }]", val)]),
        raw("}"),
    ]
}

fn draw_bottom_bar(frame: &mut Frame, app: &App, area: Rect) {
    let hint_style = Style::new().fg(DIM);
    let key_style = Style::new().fg(ACCENT);
    let mut spans: Vec<Span> = Vec::new();
    if let Some((msg, _, is_err)) = &app.status {
        let style = if *is_err { Style::new().fg(RED).bold() } else { Style::new().fg(ACCENT) };
        spans.push(Span::styled(format!(" {msg}"), style));
    } else {
        let hints: &[(&str, &str)] = match (app.focus, app.pane) {
            (Focus::List, Pane::Containers) => &[("j/k", "move"), ("enter", "focus"), ("space", "actions"), ("/", "filter"), ("f", "zoom"), ("?", "help")],
            (Focus::List, _) => &[("j/k", "move"), ("enter", "focus"), ("space", "actions"), ("/", "filter"), ("?", "help")],
            (Focus::Detail, _) => &[("j/k", "scroll"), ("l/i", "tabs"), ("F", "follow"), ("esc", "back"), ("f", "zoom")],
        };
        for (k, v) in hints {
            spans.push(Span::styled(format!(" {k}"), key_style));
            spans.push(Span::styled(format!(" {v} "), hint_style));
        }
    }
    // status cluster, right-aligned
    let cluster = format!("● service  ⌘ container 1.3.0  {} ", SPINNER[app.spinner]);
    let pad = (area.width as usize).saturating_sub(spans.iter().map(|s| s.content.chars().count()).sum::<usize>() + cluster.chars().count());
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled("● ", Style::new().fg(ACCENT)));
    spans.push(Span::styled("service  ", hint_style));
    spans.push(Span::styled("container 1.3.0  ", hint_style));
    spans.push(Span::styled(SPINNER[app.spinner].to_string(), Style::new().fg(DIM)));
    spans.push(Span::raw(" "));
    frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(Color::Rgb(0x11, 0x14, 0x1c))), area);
}

fn draw_action_menu(frame: &mut Frame, app: &App, body: Rect, bottom: Rect) {
    let danger = Style::new().fg(RED);
    let normal = Style::new().fg(TEXT);
    let items: Vec<(&str, &str, bool)> = match app.pane {
        Pane::Containers => {
            let running = app.selected_container().map(|c| c.running).unwrap_or(false);
            if running {
                vec![("s", "stop", false), ("r", "restart", false), ("K", "kill", true), ("d", "delete", true), ("P", "prune stopped", true), ("e", "exec shell", false), ("l", "logs", false), ("i", "inspect", false)]
            } else {
                vec![("s", "start", false), ("d", "delete", true), ("P", "prune stopped", true), ("i", "inspect", false)]
            }
        }
        Pane::Images => vec![("u", "pull by reference", false), ("d", "delete", true), ("P", "prune unused", true)],
        Pane::Volumes => vec![("d", "delete", true), ("P", "prune unreferenced", true)],
    };
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
        .border_style(Style::new().fg(ACCENT))
        .title(Span::styled(" actions ", Style::new().fg(ACCENT).bold()))
        .style(Style::new().bg(PANEL));
    let lines: Vec<Line> = items
        .iter()
        .map(|(k, label, destructive)| {
            let style = if *destructive { danger } else { normal };
            Line::from(vec![
                Span::styled(format!("  {k}  "), Style::new().fg(ACCENT).bold()),
                Span::styled(label.to_string(), style),
                if *destructive { Span::styled("  (confirms)", Style::new().fg(DIM)) } else { Span::raw("") },
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_confirm(frame: &mut Frame, command: &str, action: PendingKind) {
    let w = (command.len() as u16 + 8).max(44).min(frame.area().width);
    let area = centered(frame.area(), w, 7);
    frame.render_widget(Clear, area);
    let title = match action {
        PendingKind::Prune => " confirm prune ",
        PendingKind::Kill => " confirm kill ",
        _ => " confirm ",
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(RED))
        .title(Span::styled(title, Style::new().fg(RED).bold()))
        .style(Style::new().bg(PANEL));
    let lines = vec![
        Line::raw(""),
        Line::from(vec![Span::raw("  $ "), Span::styled(command.to_string(), Style::new().fg(YELLOW).bold())]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  [y]", Style::new().fg(ACCENT).bold()),
            Span::raw(" run   "),
            Span::styled("[esc]", Style::new().fg(DIM).bold()),
            Span::raw(" cancel"),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_pull_input(frame: &mut Frame, text: &str) {
    let area = centered(frame.area(), 50, 5);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
        .title(Span::styled(" pull image ", Style::new().fg(ACCENT).bold()))
        .style(Style::new().bg(PANEL));
    let lines = vec![
        Line::from(vec![Span::raw(" reference: "), Span::styled(format!("{text}▏"), Style::new().fg(TEXT))]),
        Line::from(Span::styled(" enter pulls · esc cancels — progress streams in detail pane", Style::new().fg(DIM))),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_help(frame: &mut Frame) {
    let area = centered(frame.area(), 68, 22);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
        .title(Line::from(gradient_spans(" keys ", true)))
        .style(Style::new().bg(PANEL));
    let g = |s: &str| Line::from(Span::styled(s.to_string(), Style::new().fg(ACCENT).bold()));
    let k = |key: &str, desc: &str| {
        Line::from(vec![
            Span::styled(format!("  {key:<12}"), Style::new().fg(YELLOW)),
            Span::raw(desc.to_string()),
        ])
    };
    let lines = vec![
        g(" global"),
        k("1/2/3, tab", "switch pane (containers / images / volumes)"),
        k("f", "zoom focused pane"),
        k("m", "message log"),
        k("b", "dismiss version banner"),
        k("q", "quit"),
        g(" list"),
        k("j/k g/G", "move / top / bottom"),
        k("/", "fuzzy filter (esc clears)"),
        k("enter", "focus detail pane"),
        k("space", "action menu"),
        g(" detail"),
        k("l / i", "logs / inspect tab"),
        k("F", "toggle follow"),
        k("pgup/pgdn", "scroll without focus"),
        k("esc", "back to list"),
        g(" prototype sims"),
        k("F1", "toggle ambient effect"),
        k("F2", "toggle service-down takeover"),
        k("F3", "simulate external stop"),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_message_log(frame: &mut Frame, app: &App) {
    let full = frame.area();
    let area = Rect {
        x: full.x + 2,
        y: full.y + full.height / 2,
        width: full.width - 4,
        height: full.height / 2 - 1,
    };
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
        .title(Span::styled(" message log ", Style::new().fg(ACCENT).bold()))
        .style(Style::new().bg(PANEL));
    let lines: Vec<Line> = app
        .messages
        .iter()
        .rev()
        .flat_map(|m| m.split('\n').map(|s| Line::raw(format!(" {s}")).style(Style::new().fg(TEXT))).collect::<Vec<_>>())
        .collect();
    frame.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), area);
}

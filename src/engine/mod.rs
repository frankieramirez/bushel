//! The headless core: owns `AppState`, the poller cadence, the action queue,
//! and the log follower. Knows nothing about rendering — tests inject events
//! and assert state.

pub mod event;
pub mod state;

use std::collections::HashMap;
use std::time::Instant;

use tokio::sync::mpsc;

use crate::client::{self, CliError, Client};
use crate::runner::{KillHandle, Runner, StreamEvent};

pub use event::{AppEvent, Command};
pub use state::*;

/// Poll cadence bookkeeping: containers every tick (1s); images/volumes every
/// Nth tick, on pane entry, and after mutating actions; service probe every
/// 2nd tick while down.
pub const SLOW_POLL_TICKS: u64 = 10;
pub const PROBE_TICKS: u64 = 2;

pub struct Engine<R: Runner> {
    pub state: AppState,
    client: Client<R>,
    tx: mpsc::Sender<AppEvent>,

    stats_prev: HashMap<String, StatsSnapshot>,
    follower: Option<(String, KillHandle)>,
    follow_buffer: Vec<String>,
    pull_kill: Option<KillHandle>,
    service_kill: Option<KillHandle>,

    poll_inflight: bool,
    probe_inflight: bool,
    images_dirty: bool,
    volumes_dirty: bool,
    service_down: bool,
}

impl<R: Runner> Engine<R> {
    pub fn new(client: Client<R>, tx: mpsc::Sender<AppEvent>, no_splash: bool) -> Self {
        Self {
            state: AppState::new(no_splash),
            client,
            tx,
            stats_prev: HashMap::new(),
            follower: None,
            follow_buffer: Vec::new(),
            pull_kill: None,
            service_kill: None,
            poll_inflight: false,
            probe_inflight: false,
            images_dirty: false,
            volumes_dirty: false,
            service_down: false,
        }
    }

    /// Startup probes: version check, service status, and the initial lists.
    pub fn start(&mut self) {
        self.spawn_version_check();
        self.spawn_probe();
        self.spawn_containers_poll();
        self.images_dirty = true;
        self.volumes_dirty = true;
        self.refresh_dirty();
    }

    /// One poll tick (1s cadence, driven by the outer loop's interval).
    pub fn on_tick(&mut self) {
        self.state.tick += 1;
        if let Some(t) = &self.state.toast {
            if t.at.elapsed().as_secs() >= 4 {
                self.state.toast = None;
            }
        }
        if self.service_down {
            // entity polling stops; probe every 2s until recovery
            if self.state.tick % PROBE_TICKS == 0 && !self.state.service_starting {
                self.spawn_probe();
            }
            return;
        }
        self.spawn_containers_poll();
        if self.state.containers.iter().any(|c| c.is_running()) {
            self.spawn_stats_poll();
        }
        if self.state.tick % SLOW_POLL_TICKS == 0 {
            self.images_dirty = true;
            self.volumes_dirty = true;
        }
        self.refresh_dirty();
    }

    // ---- task spawning ------------------------------------------------------

    fn spawn_containers_poll(&mut self) {
        if self.poll_inflight {
            return;
        }
        self.poll_inflight = true;
        self.state.last_poll_at = Some(Instant::now());
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(AppEvent::Containers(client.list_containers().await))
                .await;
        });
    }

    fn spawn_stats_poll(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(AppEvent::Stats(client.stats().await)).await;
        });
    }

    fn refresh_dirty(&mut self) {
        if self.images_dirty {
            self.images_dirty = false;
            let client = self.client.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(AppEvent::Images(client.list_images().await)).await;
            });
        }
        if self.volumes_dirty {
            self.volumes_dirty = false;
            let client = self.client.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let _ = tx
                    .send(AppEvent::Volumes(client.list_volumes().await))
                    .await;
            });
        }
    }

    fn spawn_probe(&mut self) {
        if self.probe_inflight {
            return;
        }
        self.probe_inflight = true;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(AppEvent::ServiceProbe(client.system_status().await))
                .await;
        });
    }

    fn spawn_version_check(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(AppEvent::VersionChecked(client.version().await))
                .await;
        });
    }

    fn spawn_action(&mut self, kind: ActionKind, id: String, args: Vec<String>) {
        let command = format!("container {}", args.join(" "));
        self.state.log_message(format!("$ {command}"));
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = client.run_action(&args).await.map(|_| ());
            let _ = tx
                .send(AppEvent::ActionDone {
                    id,
                    kind,
                    command,
                    result,
                })
                .await;
        });
    }

    /// Synthetic restart: stop then start, one pending action, one ActionDone.
    fn spawn_restart(&mut self, id: String) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        let stop = Client::<R>::stop_args(&id);
        let start = Client::<R>::start_args(&id);
        let command = format!(
            "container {} && container {}",
            stop.join(" "),
            start.join(" ")
        );
        self.state.log_message(format!("$ {command}"));
        tokio::spawn(async move {
            let result = match client.run_action(&stop).await {
                Ok(_) => client.run_action(&start).await.map(|_| ()),
                Err(e) => Err(e),
            };
            let _ = tx
                .send(AppEvent::ActionDone {
                    id,
                    kind: ActionKind::Restart,
                    command,
                    result,
                })
                .await;
        });
    }

    // ---- log follower ------------------------------------------------------------

    /// The follower lives only while the Logs tab shows a running container.
    fn sync_follower(&mut self) {
        let desired: Option<String> = if self.state.screen == Screen::Main
            && self.state.pane == Pane::Containers
            && self.state.detail_tab == DetailTab::Logs
        {
            self.state
                .selected_container()
                .filter(|c| c.is_running())
                .map(|c| c.id.clone())
        } else {
            None
        };

        let current = self.follower.as_ref().map(|(id, _)| id.clone());
        if current == desired {
            return;
        }
        if let Some((_, kill)) = self.follower.take() {
            kill.kill();
        }
        self.follow_buffer.clear();
        self.state.log_lines.clear();
        self.state.log_owner = desired.clone();
        self.state.follow_ended = false;
        self.state.logs_loading = desired.is_some();

        let Some(id) = desired else { return };

        // backlog first …
        let client = self.client.clone();
        let tx = self.tx.clone();
        let backlog_id = id.clone();
        tokio::spawn(async move {
            let (lines, error) = match client.logs_backlog(&backlog_id).await {
                Ok(lines) => (lines, None),
                Err(e) => (Vec::new(), Some(e)),
            };
            let _ = tx
                .send(AppEvent::LogBacklog {
                    id: backlog_id,
                    lines,
                    error,
                })
                .await;
        });

        // … while the follow stream buffers behind it.
        match self.client.spawn_follow(&id) {
            Ok((mut rx, kill)) => {
                self.follower = Some((id.clone(), kill));
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    while let Some(ev) = rx.recv().await {
                        let msg = match ev {
                            StreamEvent::Stdout(line) | StreamEvent::Stderr(line) => {
                                AppEvent::LogLine {
                                    id: id.clone(),
                                    line,
                                }
                            }
                            StreamEvent::Exit(_) => AppEvent::FollowExited { id: id.clone() },
                        };
                        if tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                });
            }
            Err(e) => {
                self.state
                    .toast(format!("logs -f failed to spawn: {e}"), true);
                self.state.logs_loading = false;
            }
        }
    }

    fn ensure_inspect(&mut self) {
        let target: Option<(Pane, String)> = match self.state.pane {
            Pane::Containers if self.state.detail_tab == DetailTab::Inspect => self
                .state
                .selected_container()
                .map(|c| (Pane::Containers, c.id.clone())),
            Pane::Images => self
                .state
                .selected_image()
                .map(|i| (Pane::Images, i.reference.clone())),
            Pane::Volumes => self
                .state
                .selected_volume()
                .map(|v| (Pane::Volumes, v.name.clone())),
            _ => None,
        };
        let Some((pane, id)) = target else { return };
        if self.state.inspect_cache.contains_key(&id)
            || self.state.inspect_loading.as_deref() == Some(&id)
        {
            return;
        }
        self.state.inspect_loading = Some(id.clone());
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = match pane {
                Pane::Containers => client.inspect_container(&id).await,
                Pane::Images => client.inspect_image(&id).await,
                Pane::Volumes => client.inspect_volume(&id).await,
            };
            let _ = tx.send(AppEvent::InspectLoaded { id, result }).await;
        });
    }

    // ---- update loop: task events -----------------------------------------------

    pub fn apply(&mut self, event: AppEvent) {
        match event {
            AppEvent::Containers(Ok(list)) => {
                self.poll_inflight = false;
                self.state.parse_failures = 0;
                self.state.degraded = false;
                let (diffs, external) = self.state.update_containers(&list);
                for d in diffs {
                    self.state.log_message(d);
                }
                for id in external {
                    self.state.toast(format!("{id} stopped externally"), false);
                }
                self.announce_confirmations();
                self.state.recompute_in_use();
                self.maybe_dissolve_splash();
                self.sync_follower();
                self.ensure_inspect();
            }
            AppEvent::Containers(Err(e)) => {
                self.poll_inflight = false;
                self.on_poll_error(e, true);
            }
            AppEvent::Images(Ok(list)) => {
                self.state.update_images(&list);
                self.announce_confirmations();
                self.ensure_inspect();
            }
            AppEvent::Images(Err(e)) => self.on_poll_error(e, false),
            AppEvent::Volumes(Ok(list)) => {
                self.state.update_volumes(&list);
                self.announce_confirmations();
                self.ensure_inspect();
            }
            AppEvent::Volumes(Err(e)) => self.on_poll_error(e, false),
            AppEvent::Stats(Ok(stats)) => {
                self.stats_prev = self
                    .state
                    .apply_stats(&stats, &self.stats_prev, Instant::now());
            }
            AppEvent::Stats(Err(_)) => {} // stats are best-effort garnish
            AppEvent::ServiceProbe(result) => {
                self.probe_inflight = false;
                match result {
                    Ok(s) if s.is_running() => {
                        if self.service_down || self.state.screen == Screen::ServiceDown {
                            self.service_down = false;
                            if self.state.screen == Screen::ServiceDown {
                                self.state.screen = Screen::Main;
                            }
                            self.state.toast("container system service is up", false);
                            self.spawn_containers_poll();
                            self.images_dirty = true;
                            self.volumes_dirty = true;
                            self.refresh_dirty();
                        }
                    }
                    Ok(_) | Err(CliError::ServiceDown { .. }) => self.enter_service_down(),
                    Err(e) => {
                        self.state
                            .log_message(format!("system status probe failed: {}", e.raw()));
                    }
                }
            }
            AppEvent::VersionChecked(Ok(line)) => {
                let line = line.trim().to_string();
                self.state.cli_version = client::version::parse(&line)
                    .map(|(a, b, c)| format!("{a}.{b}.{c}"))
                    .or_else(|| Some(line.clone()));
                if !client::version::is_tested(&line) {
                    self.state.version_banner = Some(format!(
                        "container CLI {} detected — bushel is tested against {}",
                        self.state.cli_version.as_deref().unwrap_or("?"),
                        client::version::tested_range(),
                    ));
                }
            }
            AppEvent::VersionChecked(Err(e)) => {
                self.state
                    .log_message(format!("version check failed: {}", e.raw()));
            }
            AppEvent::ActionDone {
                id,
                kind,
                command,
                result,
            } => {
                self.on_action_done(id, kind, command, result);
            }
            AppEvent::LogBacklog { id, lines, error } => {
                if self.state.log_owner.as_deref() == Some(&id) {
                    self.state.log_lines = lines;
                    for l in std::mem::take(&mut self.follow_buffer) {
                        self.state.push_log_line(l);
                    }
                    self.state.logs_loading = false;
                    if let Some(e) = error {
                        self.state
                            .log_message(format!("logs backlog failed: {}", e.raw()));
                    }
                }
            }
            AppEvent::LogLine { id, line } => {
                if self.state.log_owner.as_deref() == Some(&id) {
                    if self.state.logs_loading {
                        self.follow_buffer.push(line);
                    } else {
                        self.state.push_log_line(line);
                    }
                }
            }
            AppEvent::FollowExited { id } => {
                if self.state.log_owner.as_deref() == Some(&id) {
                    self.state.follow_ended = true;
                }
            }
            AppEvent::InspectLoaded { id, result } => {
                if self.state.inspect_loading.as_deref() == Some(&id) {
                    self.state.inspect_loading = None;
                }
                match result {
                    Ok(json) => {
                        self.state.inspect_cache.insert(id, json);
                    }
                    Err(e) => {
                        self.state
                            .log_message(format!("inspect {id} failed: {}", e.raw()));
                        self.state
                            .inspect_cache
                            .insert(id, format!("inspect failed: {}", e.gist()));
                    }
                }
            }
            AppEvent::PullLine { reference, line } => {
                if let Some(p) = &mut self.state.pull {
                    if p.reference == reference {
                        p.lines.push(line);
                        if p.lines.len() > 500 {
                            let excess = p.lines.len() - 500;
                            p.lines.drain(..excess);
                        }
                    }
                }
            }
            AppEvent::PullDone { reference, code } => {
                if self
                    .state
                    .pull
                    .as_ref()
                    .is_some_and(|p| p.reference == reference)
                {
                    let lines = self.state.pull.take().map(|p| p.lines).unwrap_or_default();
                    self.pull_kill = None;
                    if code == 0 {
                        self.state.toast(format!("pulled {reference}"), false);
                        self.images_dirty = true;
                        self.refresh_dirty();
                    } else {
                        let gist = lines
                            .last()
                            .cloned()
                            .unwrap_or_else(|| format!("exit {code}"));
                        self.state
                            .log_message(format!("pull {reference} failed:\n{}", lines.join("\n")));
                        self.state.toast(format!("pull failed: {gist}"), true);
                    }
                }
            }
            AppEvent::ServiceStartLine(line) => {
                self.state.service_output.push(line);
            }
            AppEvent::ServiceStartExited(code) => {
                self.state.service_starting = false;
                self.service_kill = None;
                if code != 0 {
                    self.state
                        .toast(format!("service start exited {code}"), true);
                    self.state.log_message(self.state.service_output.join("\n"));
                }
                self.spawn_probe();
            }
        }
    }

    /// `counts_toward_degraded`: only the per-tick containers poll drives the
    /// degraded banner; images/volumes failures just log.
    fn on_poll_error(&mut self, e: CliError, counts_toward_degraded: bool) {
        match e {
            CliError::ServiceDown { .. } => self.enter_service_down(),
            CliError::ParseFailure { raw } => {
                // keep last good state; degraded banner only after 3 straight failures
                self.state.log_message(format!("poll parse failure: {raw}"));
                if counts_toward_degraded {
                    self.state.parse_failures += 1;
                    if self.state.parse_failures >= DEGRADED_THRESHOLD {
                        self.state.degraded = true;
                    }
                }
            }
            other => {
                self.state
                    .log_message(format!("poll failed: {}", other.raw()));
            }
        }
        if self.state.screen == Screen::Splash {
            self.state.screen = Screen::Main;
        }
    }

    /// Dissolve the splash once data has landed — and, on the very first launch,
    /// once the dwell has elapsed. Called on poll results and every render tick.
    pub fn maybe_dissolve_splash(&mut self) {
        if self.state.screen == Screen::Splash && self.state.splash_may_dissolve() {
            self.state.screen = Screen::Main;
        }
    }

    fn announce_confirmations(&mut self) {
        for (id, kind) in self.state.take_confirmations() {
            self.state
                .toast(format!("{} {id}", kind.past_tense()), false);
        }
    }

    fn enter_service_down(&mut self) {
        if !self.service_down {
            self.service_down = true;
            self.state.service_output.clear();
            self.state
                .log_message("service down: entity polling stopped, probing every 2s");
        }
        self.state.screen = Screen::ServiceDown;
        self.sync_follower(); // kills any live follower
    }

    fn on_action_done(
        &mut self,
        id: String,
        kind: ActionKind,
        command: String,
        result: Result<(), CliError>,
    ) {
        let prune = matches!(
            kind,
            ActionKind::PruneContainers | ActionKind::PruneImages | ActionKind::PruneVolumes
        );
        match result {
            Ok(()) => {
                if prune {
                    self.state.activity = None;
                    self.state.toast(format!("done: {command}"), false);
                } else {
                    // the outcome is announced when a poll confirms it, not here
                    self.state.set_pending(
                        &id,
                        Some(Pending {
                            kind,
                            phase: PendingPhase::Confirming(CONFIRM_TICKS),
                        }),
                    );
                    self.state
                        .log_message(format!("$ {command} → ok, awaiting poll confirmation"));
                }
                // poll immediately so the outcome lands fast
                match kind {
                    ActionKind::DeleteImage | ActionKind::PruneImages => self.images_dirty = true,
                    ActionKind::DeleteVolume | ActionKind::PruneVolumes => {
                        self.volumes_dirty = true
                    }
                    _ => {}
                }
                self.spawn_containers_poll();
                self.refresh_dirty();
                self.state.inspect_cache.remove(&id);
            }
            Err(e) => {
                if prune {
                    self.state.activity = None;
                } else {
                    self.state.set_pending(&id, None);
                }
                self.state.log_message(format!("$ {command}\n{}", e.raw()));
                match e {
                    CliError::NotFound { .. } => {
                        // stale row — a poll will remove it; status-bar notice only
                        self.state.toast(format!("{id}: already gone"), false);
                        self.spawn_containers_poll();
                    }
                    other => self.state.toast(other.gist(), true),
                }
            }
        }
        self.sync_follower();
    }

    // ---- update loop: user commands ------------------------------------------------

    pub fn dispatch(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => self.state.quit = true,
            Command::SkipSplash => {
                if self.state.screen == Screen::Splash {
                    self.state.screen = Screen::Main;
                }
            }
            Command::SwitchPane(pane) => self.switch_pane(pane),
            Command::NextPane => self.switch_pane(self.state.pane.next()),
            Command::FocusDetail => {
                self.state.focus = Focus::Detail;
                self.ensure_inspect();
            }
            Command::Back => {
                if self.state.focus == Focus::Detail {
                    self.state.focus = Focus::List;
                } else if !self.state.filter.is_empty() || self.state.filter_input {
                    self.state.filter.clear();
                    self.state.filter_input = false;
                } else if self.state.zoom {
                    self.state.zoom = false;
                }
            }
            Command::ToggleZoom => self.state.zoom = !self.state.zoom,
            Command::SetDetailTab(tab) => {
                if self.state.pane == Pane::Containers && self.state.detail_tab != tab {
                    self.state.detail_tab = tab;
                    self.state.detail_scroll = 0;
                    self.sync_follower();
                    self.ensure_inspect();
                }
            }
            Command::Move(delta) => {
                self.state.move_selection(delta);
                self.on_selection_change();
            }
            Command::Top => {
                self.state.select_edge(true);
                self.on_selection_change();
            }
            Command::Bottom => {
                self.state.select_edge(false);
                self.on_selection_change();
            }
            Command::StartFilter => {
                self.state.filter_input = true;
            }
            Command::FilterChar(c) => {
                self.state.filter.push(c);
                self.state.clamp_selection();
            }
            Command::FilterBackspace => {
                self.state.filter.pop();
            }
            Command::FilterCommit => self.state.filter_input = false,
            Command::OpenActionMenu => self.state.overlay = Overlay::ActionMenu,
            Command::OpenHelp => {
                self.state.overlay = Overlay::Help;
                self.state.help_scroll = 0;
            }
            Command::OpenMessageLog => self.state.overlay = Overlay::MessageLog,
            Command::CloseOverlay => self.state.overlay = Overlay::None,
            Command::DismissBanner => self.state.version_banner = None,
            Command::Run(action) => self.run_ui_action(action),
            Command::ConfirmYes => {
                if let Overlay::Confirm { action, target, .. } = self.state.overlay.clone() {
                    self.state.overlay = Overlay::None;
                    self.run_confirmed(action, target);
                }
            }
            Command::OverlayChar(c) => match &mut self.state.overlay {
                Overlay::ActionMenu => {
                    if let Some(item) = self.state.available_actions().iter().find(|i| i.key == c) {
                        let action = item.action;
                        self.state.overlay = Overlay::None;
                        self.run_ui_action(action);
                    } else if c == ' ' {
                        self.state.overlay = Overlay::None;
                    }
                }
                Overlay::PullInput { text } => text.push(c),
                _ => {}
            },
            Command::OverlayBackspace => {
                if let Overlay::PullInput { text } = &mut self.state.overlay {
                    text.pop();
                }
            }
            Command::OverlaySubmit => {
                if let Overlay::PullInput { text } = &self.state.overlay {
                    let reference = text.trim().to_string();
                    self.state.overlay = Overlay::None;
                    if !reference.is_empty() {
                        self.start_pull(reference);
                    }
                }
            }
            Command::ScrollDetail(delta) => {
                let s = &mut self.state.detail_scroll;
                *s = if delta < 0 {
                    s.saturating_sub((-delta) as u16)
                } else {
                    s.saturating_add(delta as u16)
                };
                if self.state.pane == Pane::Containers
                    && self.state.detail_tab == DetailTab::Logs
                    && delta < 0
                {
                    self.state.follow = false; // scrolling up pauses the tail
                }
            }
            Command::SetDetailScroll(v) => {
                self.state.detail_scroll = v;
                self.state.follow = false;
            }
            Command::SetHelpScroll(v) => self.state.help_scroll = v,
            Command::ScrollTop => self.state.detail_scroll = 0,
            Command::ScrollBottom => self.state.detail_scroll = u16::MAX,
            Command::ToggleFollow => self.state.follow = !self.state.follow,
            Command::ToggleWrap => self.state.wrap = !self.state.wrap,
            Command::StartService => self.start_service(),
        }
    }

    fn switch_pane(&mut self, pane: Pane) {
        if self.state.pane == pane {
            return;
        }
        self.state.pane = pane;
        self.state.detail_scroll = 0;
        self.state.focus = Focus::List;
        // images/volumes refresh on pane entry
        match pane {
            Pane::Images => self.images_dirty = true,
            Pane::Volumes => self.volumes_dirty = true,
            Pane::Containers => {}
        }
        self.refresh_dirty();
        self.sync_follower();
        self.ensure_inspect();
    }

    fn on_selection_change(&mut self) {
        self.state.detail_scroll = 0;
        self.sync_follower();
        self.ensure_inspect();
    }

    fn run_ui_action(&mut self, action: UiAction) {
        match (self.state.pane, action) {
            (_, UiAction::LogsTab) => self.dispatch(Command::SetDetailTab(DetailTab::Logs)),
            (_, UiAction::InspectTab) => self.dispatch(Command::SetDetailTab(DetailTab::Inspect)),
            (Pane::Containers, UiAction::Start | UiAction::Stop | UiAction::Restart) => {
                let Some(c) = self.state.selected_container() else {
                    return;
                };
                if c.pending.is_some() {
                    self.state
                        .toast(format!("{}: action already pending", c.id), true);
                    return;
                }
                let id = c.id.clone();
                let running = c.is_running();
                let kind = match action {
                    UiAction::Restart => ActionKind::Restart,
                    _ if running => ActionKind::Stop,
                    _ => ActionKind::Start,
                };
                self.state.set_pending(
                    &id,
                    Some(Pending {
                        kind,
                        phase: PendingPhase::InFlight,
                    }),
                );
                match kind {
                    ActionKind::Restart => self.spawn_restart(id),
                    ActionKind::Stop => {
                        self.spawn_action(kind, id.clone(), Client::<R>::stop_args(&id))
                    }
                    _ => self.spawn_action(kind, id.clone(), Client::<R>::start_args(&id)),
                }
            }
            (Pane::Containers, UiAction::Kill) => {
                let Some(c) = self.state.selected_container() else {
                    return;
                };
                if !c.is_running() {
                    return;
                }
                self.open_confirm(
                    ActionKind::Kill,
                    c.id.clone(),
                    Client::<R>::kill_args(&c.id),
                );
            }
            (Pane::Containers, UiAction::Delete) => {
                let Some(c) = self.state.selected_container() else {
                    return;
                };
                self.open_confirm(
                    ActionKind::DeleteContainer,
                    c.id.clone(),
                    Client::<R>::delete_container_args(&c.id),
                );
            }
            (Pane::Containers, UiAction::Prune) => {
                self.open_confirm(
                    ActionKind::PruneContainers,
                    String::new(),
                    Client::<R>::prune_containers_args(),
                );
            }
            (Pane::Containers, UiAction::Exec) => {
                let Some(c) = self.state.selected_container() else {
                    return;
                };
                if c.is_running() {
                    self.state.exec_request = Some(c.id.clone());
                }
            }
            (Pane::Images, UiAction::Pull) => {
                self.state.overlay = Overlay::PullInput {
                    text: String::new(),
                };
            }
            (Pane::Images, UiAction::Delete) => {
                let Some(i) = self.state.selected_image() else {
                    return;
                };
                self.open_confirm(
                    ActionKind::DeleteImage,
                    i.reference.clone(),
                    Client::<R>::delete_image_args(&i.reference),
                );
            }
            (Pane::Images, UiAction::Prune) => {
                self.open_confirm(
                    ActionKind::PruneImages,
                    String::new(),
                    Client::<R>::prune_images_args(),
                );
            }
            (Pane::Volumes, UiAction::Delete) => {
                let Some(v) = self.state.selected_volume() else {
                    return;
                };
                // in-use volumes are blocked with an error, no confirm
                if v.in_use() {
                    let name = v.name.clone();
                    let by = v.in_use_by.join(", ");
                    self.state.log_message(format!(
                        "container volume delete {name}\nblocked by bushel: volume \"{name}\" is in use by {by}"
                    ));
                    self.state
                        .toast(format!("cannot delete {name}: in use by {by}"), true);
                    return;
                }
                self.open_confirm(
                    ActionKind::DeleteVolume,
                    v.name.clone(),
                    Client::<R>::delete_volume_args(&v.name),
                );
            }
            (Pane::Volumes, UiAction::Prune) => {
                self.open_confirm(
                    ActionKind::PruneVolumes,
                    String::new(),
                    Client::<R>::prune_volumes_args(),
                );
            }
            _ => {}
        }
    }

    fn open_confirm(&mut self, action: ActionKind, target: String, args: Vec<String>) {
        // guard: one pending action per entity id
        if !target.is_empty() && self.state.pending_of(&target).is_some() {
            self.state
                .toast(format!("{target}: action already pending"), true);
            return;
        }
        let command = format!("container {}", args.join(" "));
        self.state.overlay = Overlay::Confirm {
            command,
            action,
            target,
        };
    }

    fn run_confirmed(&mut self, action: ActionKind, target: String) {
        let args = match action {
            ActionKind::Kill => Client::<R>::kill_args(&target),
            ActionKind::DeleteContainer => Client::<R>::delete_container_args(&target),
            ActionKind::PruneContainers => Client::<R>::prune_containers_args(),
            ActionKind::DeleteImage => Client::<R>::delete_image_args(&target),
            ActionKind::PruneImages => Client::<R>::prune_images_args(),
            ActionKind::DeleteVolume => Client::<R>::delete_volume_args(&target),
            ActionKind::PruneVolumes => Client::<R>::prune_volumes_args(),
            // start/stop/restart never confirm
            _ => return,
        };
        if target.is_empty() {
            // prune: bottom-bar activity with elapsed, no per-row pending
            self.state.activity = Some(Activity {
                label: format!("container {}", args.join(" ")),
                started: Instant::now(),
            });
        } else {
            self.state.set_pending(
                &target,
                Some(Pending {
                    kind: action,
                    phase: PendingPhase::InFlight,
                }),
            );
        }
        self.spawn_action(action, target, args);
    }

    fn start_pull(&mut self, reference: String) {
        // tag defaults to latest — only when the last path segment is untagged
        // (a ':' in "localhost:5000/nginx" is a registry port, not a tag)
        let last_segment = reference.rsplit('/').next().unwrap_or(&reference);
        let reference = if last_segment.contains(':') {
            reference
        } else {
            format!("{reference}:latest")
        };
        if self.state.pull.is_some() {
            self.state.toast("a pull is already running", true);
            return;
        }
        match self.client.spawn_pull(&reference) {
            Ok((mut rx, kill)) => {
                self.state.log_message(format!(
                    "$ container {}",
                    Client::<R>::pull_args(&reference).join(" ")
                ));
                self.state.pull = Some(PullState {
                    reference: reference.clone(),
                    lines: Vec::new(),
                    started: Instant::now(),
                });
                self.pull_kill = Some(kill);
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    while let Some(ev) = rx.recv().await {
                        let msg = match ev {
                            StreamEvent::Stdout(line) | StreamEvent::Stderr(line) => {
                                AppEvent::PullLine {
                                    reference: reference.clone(),
                                    line,
                                }
                            }
                            StreamEvent::Exit(code) => AppEvent::PullDone {
                                reference: reference.clone(),
                                code,
                            },
                        };
                        if tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                });
            }
            Err(e) => self.state.toast(format!("pull failed to spawn: {e}"), true),
        }
    }

    fn start_service(&mut self) {
        if self.state.service_starting {
            return;
        }
        self.state.service_starting = true;
        self.state.service_output.clear();
        self.state.service_output.push(format!(
            "$ container {}",
            Client::<R>::system_start_args().join(" ")
        ));
        match self.client.spawn_system_start() {
            Ok((mut rx, kill)) => {
                self.service_kill = Some(kill);
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    while let Some(ev) = rx.recv().await {
                        let msg = match ev {
                            StreamEvent::Stdout(line) | StreamEvent::Stderr(line) => {
                                AppEvent::ServiceStartLine(line)
                            }
                            StreamEvent::Exit(code) => AppEvent::ServiceStartExited(code),
                        };
                        if tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                });
            }
            Err(e) => {
                self.state.service_starting = false;
                self.state
                    .toast(format!("failed to spawn service start: {e}"), true);
            }
        }
    }

    // ---- exec ----------------------------------------------------------------------

    /// Called by the outer loop before suspending the TUI for exec.
    pub fn prepare_exec(&mut self) -> Vec<String> {
        let id = self.state.exec_request.take().unwrap_or_default();
        if let Some((_, kill)) = self.follower.take() {
            kill.kill();
        }
        self.state.log_owner = None;
        Client::<R>::exec_shell_args(&id)
    }

    /// Called after the TUI is restored: immediate poll, follower resync.
    pub fn after_exec(&mut self) {
        self.spawn_containers_poll();
        self.sync_follower();
        self.ensure_inspect();
    }

    /// Kill every owned subprocess (quit path).
    pub fn shutdown(&mut self) {
        if let Some((_, kill)) = self.follower.take() {
            kill.kill();
        }
        if let Some(kill) = self.pull_kill.take() {
            kill.kill();
        }
        if let Some(kill) = self.service_kill.take() {
            kill.kill();
        }
    }

    /// Test-only visibility: is a follower alive, and for whom?
    pub fn follower_id(&self) -> Option<&str> {
        self.follower.as_ref().map(|(id, _)| id.as_str())
    }
}

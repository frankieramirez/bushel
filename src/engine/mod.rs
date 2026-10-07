pub mod event;
pub mod pending;
pub mod state;

use std::collections::HashMap;
use std::time::Instant;

use std::sync::Arc;
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinHandle;

use crate::client::{self, CliError, Client};
use crate::runner::{KillHandle, Runner, StreamEvent};

pub use event::{AppEvent, Command};
use pending::{
    ActionId, ActionPlan, Observation, Outcome, OutcomeStatus, PendingActions, TagTarget, Target,
};
pub use state::*;

pub const SLOW_POLL_TICKS: u64 = 10;
pub const PROBE_TICKS: u64 = 2;

struct StopAttribution {
    completion_floor: u64,
    completed_at: Instant,
}

pub struct Engine<R: Runner> {
    pub state: AppState,
    client: Client<R>,
    tx: mpsc::Sender<AppEvent>,

    stats_prev: HashMap<String, StatsSnapshot>,
    follower: Option<(String, KillHandle)>,
    follow_buffer: Vec<String>,
    follow_generation: u64,
    backlog_task: Option<JoinHandle<()>>,
    inspect_task: Option<JoinHandle<()>>,
    inspect_current: Option<(Target, u64)>,
    inspect_sequence: u64,
    inspect_semaphore: Arc<Semaphore>,
    stats_inflight: Option<u64>,
    stats_applied: u64,
    stats_taken_at: Option<Instant>,
    pull_kill: Option<KillHandle>,
    service_kill: Option<KillHandle>,

    pending: PendingActions,
    stop_attribution: HashMap<String, StopAttribution>,
    confirmation: Option<ActionPlan>,
    tag_source: Option<(String, Option<String>)>,
    prune: Option<(u64, Pane)>,
    prune_generation: u64,
    prune_command: Option<String>,
    poll_sequence: u64,
    applied_poll: [u64; Pane::COUNT],
    poll_inflight: bool,
    probe_inflight: bool,
    images_dirty: bool,
    volumes_dirty: bool,
    networks_dirty: bool,
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
            follow_generation: 0,
            backlog_task: None,
            inspect_task: None,
            inspect_current: None,
            inspect_sequence: 0,
            inspect_semaphore: Arc::new(Semaphore::new(1)),
            stats_inflight: None,
            stats_applied: 0,
            stats_taken_at: None,
            pull_kill: None,
            service_kill: None,
            pending: PendingActions::default(),
            stop_attribution: HashMap::new(),
            confirmation: None,
            tag_source: None,
            prune: None,
            prune_generation: 0,
            prune_command: None,
            poll_sequence: 0,
            applied_poll: [0; Pane::COUNT],
            poll_inflight: false,
            probe_inflight: false,
            images_dirty: false,
            volumes_dirty: false,
            networks_dirty: false,
            service_down: false,
        }
    }

    pub fn start(&mut self) {
        self.spawn_version_check();
        self.spawn_probe();
        self.spawn_containers_poll();
        self.images_dirty = true;
        self.volumes_dirty = true;
        self.networks_dirty = true;
        self.refresh_dirty();
    }

    pub fn on_tick(&mut self) {
        self.state.tick += 1;
        self.expire_stop_attribution();
        self.state.degraded = self.state.poll_health.degraded(self.state.tick);
        if let Some(t) = &self.state.toast {
            if t.at.elapsed().as_secs() >= 4 {
                self.state.toast = None;
            }
        }
        if self.service_down {
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
            self.networks_dirty = true;
        }
        self.refresh_dirty();
    }

    fn spawn_containers_poll(&mut self) {
        if self.poll_inflight {
            return;
        }
        self.poll_inflight = true;
        let sequence = self.next_poll();
        self.state.last_poll_at = Some(Instant::now());
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(AppEvent::Containers(
                    sequence,
                    client.list_containers().await,
                ))
                .await;
        });
    }

    fn spawn_stats_poll(&mut self) {
        if self.stats_inflight.is_some() {
            return;
        }
        let sequence = self.next_poll();
        self.stats_inflight = Some(sequence);
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = client.stats().await;
            let taken_at = Instant::now();
            let _ = tx
                .send(AppEvent::Stats {
                    sequence,
                    taken_at,
                    result,
                })
                .await;
        });
    }

    fn refresh_dirty(&mut self) {
        if self.images_dirty {
            self.images_dirty = false;
            let sequence = self.next_poll();
            let client = self.client.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let _ = tx
                    .send(AppEvent::Images(sequence, client.list_images().await))
                    .await;
            });
        }
        if self.volumes_dirty {
            self.volumes_dirty = false;
            let sequence = self.next_poll();
            let client = self.client.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let _ = tx
                    .send(AppEvent::Volumes(sequence, client.list_volumes().await))
                    .await;
            });
        }
        if self.networks_dirty {
            self.networks_dirty = false;
            let sequence = self.next_poll();
            let client = self.client.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let _ = tx
                    .send(AppEvent::Networks(sequence, client.list_networks().await))
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

    fn next_poll(&mut self) -> u64 {
        self.poll_sequence += 1;
        self.poll_sequence
    }

    fn spawn_plan(&mut self, action_id: ActionId, plan: ActionPlan) {
        self.state.log_message(format!("$ {}", plan.command()));
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut result = Ok(());
            let mut completed_steps = 0;
            for args in &plan.commands {
                if let Err(error) = client.run_action(args).await {
                    result = Err(error);
                    break;
                }
                completed_steps += 1;
            }
            let _ = tx
                .send(AppEvent::ActionDone {
                    action_id,
                    result,
                    completed_steps,
                })
                .await;
        });
    }

    fn cancel_backlog(&mut self) {
        if let Some(task) = self.backlog_task.take() {
            task.abort();
        }
    }

    fn stop_follower(&mut self) {
        if let Some((_, kill)) = self.follower.take() {
            kill.kill();
        }
    }

    fn sync_follower(&mut self) {
        let selected = if self.state.screen == Screen::Main
            && self.state.pane == Pane::Containers
            && self.state.detail_tab == DetailTab::Logs
        {
            self.state
                .selected_container()
                .map(|c| (c.id.clone(), c.is_running()))
        } else {
            None
        };
        let owner = selected.as_ref().map(|(id, _)| id.clone());
        if self.state.log_owner != owner {
            self.stop_follower();
            self.cancel_backlog();
            self.follow_generation += 1;
            self.follow_buffer.clear();
            self.state.log_lines.clear();
            self.state.log_owner = owner;
            self.state.logs_loading = false;
            self.state.follow_ended = false;
        }
        let Some((id, running)) = selected else {
            return;
        };
        if !running {
            // Stop live following, retaining the tail and any already-started backlog.
            // Fresh stopped-container reads await native CLI compatibility evidence.
            self.stop_follower();
            self.state.follow_ended = true;
            return;
        }
        if self
            .follower
            .as_ref()
            .is_some_and(|(current, _)| current == &id)
        {
            return;
        }
        self.cancel_backlog();
        self.follow_generation += 1;
        let generation = self.follow_generation;
        self.follow_buffer.clear();
        self.state.log_lines.clear();
        self.state.follow_ended = false;
        self.state.logs_loading = true;

        let client = self.client.clone();
        let tx = self.tx.clone();
        let backlog_id = id.clone();
        self.backlog_task = Some(tokio::spawn(async move {
            let (lines, error) = match client.logs_backlog(&backlog_id).await {
                Ok(lines) => (lines, None),
                Err(e) => (Vec::new(), Some(e)),
            };
            let _ = tx
                .send(AppEvent::LogBacklog {
                    generation,
                    id: backlog_id,
                    lines,
                    error,
                })
                .await;
        }));

        match self.client.spawn_follow(&id) {
            Ok((mut rx, kill)) => {
                self.follower = Some((id.clone(), kill));
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    while let Some(ev) = rx.recv().await {
                        let msg = match ev {
                            StreamEvent::Stdout(line) | StreamEvent::Stderr(line) => {
                                AppEvent::LogLine {
                                    generation,
                                    id: id.clone(),
                                    line,
                                }
                            }
                            StreamEvent::Exit(_) => AppEvent::FollowExited {
                                generation,
                                id: id.clone(),
                            },
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
                self.state.follow_ended = true;
            }
        }
    }

    fn cancel_inspect(&mut self) {
        if let Some(task) = self.inspect_task.take() {
            task.abort();
        }
        self.inspect_sequence += 1;
        self.inspect_current = None;
        self.state.inspect_loading = None;
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
            Pane::Networks => self
                .state
                .selected_network()
                .map(|n| (Pane::Networks, n.name.clone())),
            _ => None,
        };
        let target = target.map(|(pane, id)| Target::new(pane, id));
        let revision = target.as_ref().map(|t| self.state.inspect_revision(t));
        if self
            .inspect_current
            .as_ref()
            .is_some_and(|(active, current_revision)| {
                Some(active) == target.as_ref() && Some(*current_revision) == revision
            })
        {
            return;
        }
        self.cancel_inspect();
        let Some(target) = target else {
            return;
        };
        let identity = self.state.inspect_identity(&target);
        if self
            .state
            .inspect_cache
            .get(&target)
            .is_some_and(|cached| cached.identity == identity)
        {
            return;
        }
        let revision = self.state.inspect_revision(&target);
        let generation = self.inspect_sequence;
        self.inspect_current = Some((target.clone(), revision));
        self.state.inspect_loading = Some(target.clone());
        self.state.inspect_errors.remove(&target);
        let client = self.client.clone();
        let tx = self.tx.clone();
        let semaphore = self.inspect_semaphore.clone();
        self.inspect_task = Some(tokio::spawn(async move {
            // Aborted tasks drop their permit before another inspect reaches Runner.
            let Ok(_permit) = semaphore.acquire_owned().await else {
                return;
            };
            let result = match target.pane {
                Pane::Containers => client.inspect_container(&target.name).await,
                Pane::Images => client.inspect_image(&target.name).await,
                Pane::Volumes => client.inspect_volume(&target.name).await,
                Pane::Networks => client.inspect_network(&target.name).await,
            };
            let _ = tx
                .send(AppEvent::InspectLoaded {
                    target,
                    generation,
                    revision,
                    identity,
                    result,
                })
                .await;
        }));
    }

    pub fn apply(&mut self, event: AppEvent) {
        match event {
            AppEvent::Containers(sequence, result) => {
                self.poll_inflight = false;
                if sequence <= self.applied_poll[Pane::Containers.index()] {
                    return;
                }
                self.applied_poll[Pane::Containers.index()] = sequence;
                match result {
                    Ok(list) => {
                        self.state.parse_failures = 0;
                        self.state.degraded = false;
                        self.state.poll_health.succeed(self.state.tick);
                        if self.state.screen == Screen::CliMissing {
                            self.state.screen = Screen::Main;
                        }
                        let rows: Vec<_> = list
                            .iter()
                            .map(|c| Observation {
                                name: c.id.clone(),
                                state: Some(c.state()),
                                digest: None,
                            })
                            .collect();
                        let (diffs, external) = self.state.update_containers(&list);
                        self.expire_stop_attribution();
                        for d in diffs {
                            self.state.log_message(d);
                        }
                        for id in external {
                            if !self.stop_attribution.contains_key(&id) {
                                self.state.toast(format!("{id} stopped externally"), false);
                            }
                        }
                        // An older in-flight poll can also observe our stop, but
                        // only a poll started after completion consumes attribution.
                        self.stop_attribution
                            .retain(|_, attribution| sequence <= attribution.completion_floor);
                        self.observe_pending(Pane::Containers, sequence, &rows);
                        self.state.recompute_in_use();
                        self.maybe_dissolve_splash();
                        self.sync_follower();
                        self.ensure_inspect();
                    }
                    Err(e) => self.on_poll_error(e, Pane::Containers),
                }
            }
            AppEvent::Images(sequence, result) => {
                if sequence <= self.applied_poll[Pane::Images.index()] {
                    return;
                }
                self.applied_poll[Pane::Images.index()] = sequence;
                match result {
                    Ok(list) => {
                        let rows: Vec<_> = list
                            .iter()
                            .map(|i| Observation {
                                name: i.reference().to_string(),
                                state: None,
                                digest: i
                                    .configuration
                                    .descriptor
                                    .as_ref()
                                    .and_then(|d| d.digest.clone()),
                            })
                            .collect();
                        self.state.update_images(&list);
                        self.observe_pending(Pane::Images, sequence, &rows);
                        self.ensure_inspect();
                    }
                    Err(e) => self.on_poll_error(e, Pane::Images),
                }
            }
            AppEvent::Volumes(sequence, result) => {
                if sequence <= self.applied_poll[Pane::Volumes.index()] {
                    return;
                }
                self.applied_poll[Pane::Volumes.index()] = sequence;
                match result {
                    Ok(list) => {
                        let rows: Vec<_> = list
                            .iter()
                            .map(|v| Observation {
                                name: v.name().to_string(),
                                state: None,
                                digest: None,
                            })
                            .collect();
                        self.state.update_volumes(&list);
                        self.observe_pending(Pane::Volumes, sequence, &rows);
                        self.ensure_inspect();
                    }
                    Err(e) => self.on_poll_error(e, Pane::Volumes),
                }
            }
            AppEvent::Networks(sequence, result) => {
                if sequence <= self.applied_poll[Pane::Networks.index()] {
                    return;
                }
                self.applied_poll[Pane::Networks.index()] = sequence;
                match result {
                    Ok(list) => {
                        self.state.update_networks(&list);
                        self.ensure_inspect();
                    }
                    Err(e) => self.on_poll_error(e, Pane::Networks),
                }
            }
            AppEvent::Stats {
                sequence,
                taken_at,
                result,
            } => {
                if self.stats_inflight == Some(sequence) {
                    self.stats_inflight = None;
                }
                if sequence <= self.stats_applied
                    || self.stats_taken_at.is_some_and(|last| taken_at <= last)
                {
                    return;
                }
                self.stats_applied = sequence;
                match result {
                    Ok(stats) => {
                        if self.state.stats_health.consecutive_failures > 0 {
                            // This sample reestablishes the cumulative baseline; old
                            // derivatives must not appear current after health clears.
                            for container in &mut self.state.containers {
                                container.cpu_percent = None;
                                container.telemetry.clear();
                            }
                        }
                        self.state.stats_health.succeed(self.state.tick);
                        self.stats_taken_at = Some(taken_at);
                        self.stats_prev =
                            self.state.apply_stats(&stats, &self.stats_prev, taken_at);
                    }
                    Err(e) => self.on_stats_error(e),
                }
            }
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
                            self.networks_dirty = true;
                            self.refresh_dirty();
                        }
                    }
                    Ok(_) | Err(CliError::ServiceDown { .. }) => self.enter_service_down(),
                    Err(e) => {
                        self.state
                            .log_message(format!("system status probe failed: {}", e.raw()));
                        if matches!(e, CliError::CliMissing { .. }) {
                            self.enter_cli_missing();
                        }
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
                if matches!(e, CliError::CliMissing { .. }) {
                    self.enter_cli_missing();
                }
            }
            AppEvent::ActionDone {
                action_id,
                result,
                completed_steps,
            } => self.on_action_done(action_id, result, completed_steps),
            AppEvent::PruneDone {
                generation,
                command,
                result,
            } => {
                if let Some((active, pane)) = self.prune {
                    if active != generation {
                        return;
                    }
                    self.prune = None;
                    self.prune_command = None;
                    self.state.activity = None;
                    match result {
                        Ok(()) => {
                            self.state.toast(format!("done: {command}"), false);
                            self.refresh_pane(pane);
                        }
                        Err(e) => {
                            self.state.log_message(format!("$ {command}\n{}", e.raw()));
                            self.state.toast(e.gist(), true);
                        }
                    }
                }
            }
            AppEvent::LogBacklog {
                generation,
                id,
                lines,
                error,
            } => {
                if generation == self.follow_generation
                    && self.state.log_owner.as_deref() == Some(&id)
                {
                    self.backlog_task = None;
                    self.state.log_lines.clear();
                    self.state.log_lines.extend(lines);
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
            AppEvent::LogLine {
                generation,
                id,
                line,
            } => {
                if generation == self.follow_generation
                    && self.state.log_owner.as_deref() == Some(&id)
                {
                    if self.state.logs_loading {
                        self.follow_buffer.push(line);
                    } else {
                        self.state.push_log_line(line);
                    }
                }
            }
            AppEvent::FollowExited { generation, id } => {
                if generation == self.follow_generation
                    && self.state.log_owner.as_deref() == Some(&id)
                {
                    self.state.follow_ended = true;
                }
            }
            AppEvent::InspectLoaded {
                target,
                generation,
                revision,
                identity,
                result,
            } => {
                if generation != self.inspect_sequence
                    || self.state.inspect_loading.as_ref() != Some(&target)
                    || revision != self.state.inspect_revision(&target)
                    || identity != self.state.inspect_identity(&target)
                {
                    self.ensure_inspect();
                    return;
                }
                self.inspect_task = None;
                self.inspect_current = None;
                self.state.inspect_loading = None;
                match result {
                    Ok(json) => {
                        self.state
                            .inspect_cache
                            .insert(target, CachedInspect { identity, json });
                    }
                    Err(e) => {
                        self.state.log_message(format!(
                            "inspect {} failed: {}",
                            target.name,
                            e.raw()
                        ));
                        self.state
                            .inspect_errors
                            .insert(target, format!("inspect failed: {}", e.gist()));
                    }
                }
            }
            AppEvent::PullLine { reference, line } => {
                if let Some(p) = &mut self.state.pull {
                    if client::canonical_reference(&p.reference)
                        == client::canonical_reference(&reference)
                    {
                        p.lines.push(line);
                        if p.lines.len() > 500 {
                            let excess = p.lines.len() - 500;
                            p.lines.drain(..excess);
                        }
                    }
                }
            }
            AppEvent::PullDone { reference, code } => {
                if self.state.pull.as_ref().is_some_and(|p| {
                    client::canonical_reference(&p.reference)
                        == client::canonical_reference(&reference)
                }) {
                    let lines = self.state.pull.take().map(|p| p.lines).unwrap_or_default();
                    self.pull_kill = None;
                    if code == 0 {
                        self.state.toast(format!("pulled {reference}"), false);
                        // Expire the pulled identity even when its typed spelling is a short alias.
                        let targets: Vec<_> = self
                            .state
                            .inspect_cache
                            .keys()
                            .chain(self.state.inspect_loading.iter())
                            .chain(self.state.inspect_errors.keys())
                            .filter(|target| {
                                target.pane == Pane::Images
                                    && client::canonical_reference(&target.name)
                                        == client::canonical_reference(&reference)
                            })
                            .cloned()
                            .collect();
                        for target in targets {
                            self.state.invalidate_inspect(&target);
                        }
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
        self.refresh_quit();
    }

    fn on_poll_error(&mut self, e: CliError, pane: Pane) {
        self.state.reads[pane.index()] = ReadStatus::Failed { gist: e.gist() };
        if pane == Pane::Containers && !matches!(e, CliError::ServiceDown { .. }) {
            self.state.poll_health.fail(e.gist());
            self.state.degraded = self.state.poll_health.degraded(self.state.tick);
        }
        match e {
            CliError::ServiceDown { .. } => self.enter_service_down(),
            CliError::CliMissing { raw } => {
                self.state.log_message(format!("poll failed: {raw}"));
                self.enter_cli_missing();
            }
            CliError::ParseFailure { raw } => {
                self.state.log_message(format!("poll parse failure: {raw}"));
                if pane == Pane::Containers {
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

    fn on_stats_error(&mut self, error: CliError) {
        self.state.stats_health.fail(error.gist());
        self.state
            .log_message(format!("stats failed: {}", error.raw()));
        // Rates cannot bridge a failed sample. Recovery starts a new baseline.
        self.stats_prev.clear();
        if self.state.stats_health.consecutive_failures >= DEGRADED_THRESHOLD {
            for container in &mut self.state.containers {
                container.cpu_percent = None;
                container.mem_bytes = None;
                container.telemetry.clear();
            }
        }
        match error {
            CliError::ServiceDown { .. } => self.enter_service_down(),
            CliError::CliMissing { .. } => self.enter_cli_missing(),
            _ => {}
        }
    }

    pub fn maybe_dissolve_splash(&mut self) {
        if self.state.screen == Screen::Splash && self.state.splash_may_dissolve() {
            self.state.screen = Screen::Main;
        }
    }

    fn observe_pending(&mut self, pane: Pane, sequence: u64, rows: &[Observation]) {
        for outcome in self.pending.observe(pane, sequence, rows) {
            self.finish_pending(outcome);
        }
        self.sync_pending();
    }

    fn sync_pending(&mut self) {
        for c in &mut self.state.containers {
            c.pending = self
                .pending
                .pending_for(&Target::new(Pane::Containers, &c.id));
        }
        for i in &mut self.state.images {
            i.pending = self
                .pending
                .pending_for(&Target::new(Pane::Images, &i.reference));
        }
        for v in &mut self.state.volumes {
            v.pending = self
                .pending
                .pending_for(&Target::new(Pane::Volumes, &v.name));
        }
    }

    fn finish_pending(&mut self, outcome: Outcome) {
        let plan = outcome.plan;
        if plan.kind == ActionKind::CreateVolume {
            self.state.remove_creating_placeholder(&plan.target.name);
        }
        match outcome.status {
            OutcomeStatus::Confirmed => {
                let name = plan
                    .tag
                    .as_ref()
                    .map(|t| t.reference.as_str())
                    .unwrap_or(&plan.target.name);
                self.state
                    .toast(format!("{} {name}", plan.kind.past_tense()), false);
            }
            OutcomeStatus::Unconfirmed => self.state.toast(
                format!(
                    "{}: command completed; outcome unconfirmed",
                    plan.target.name
                ),
                false,
            ),
            OutcomeStatus::Failed => {}
        }
    }

    fn refresh_pane(&mut self, pane: Pane) {
        match pane {
            Pane::Containers => self.spawn_containers_poll(),
            Pane::Images => self.images_dirty = true,
            Pane::Volumes => self.volumes_dirty = true,
            Pane::Networks => self.networks_dirty = true,
        }
        self.refresh_dirty();
    }

    fn enter_service_down(&mut self) {
        if !self.service_down {
            self.service_down = true;
            self.state.service_output.clear();
            self.state
                .log_message("service down: entity polling stopped, probing every 2s");
        }
        self.state.screen = Screen::ServiceDown;
        self.sync_follower();
    }

    fn enter_cli_missing(&mut self) {
        self.state.screen = Screen::CliMissing;
        self.sync_follower();
    }

    fn expire_stop_attribution(&mut self) {
        // A failed or unavailable containers poll must not keep attributing
        // future stops to this action beyond the ten-second read timeout.
        self.stop_attribution
            .retain(|_, attribution| attribution.completed_at.elapsed() < client::READ_TIMEOUT);
    }

    fn on_action_done(
        &mut self,
        action_id: ActionId,
        result: Result<(), CliError>,
        completed_steps: usize,
    ) {
        let Some(completion) = self
            .pending
            .complete(action_id, result.is_ok(), self.poll_sequence)
        else {
            return;
        };
        let plan = completion.plan;
        let command = plan.command();
        if let Some(outcome) = completion.outcome {
            self.finish_pending(outcome);
        }
        self.sync_pending();
        match result {
            Ok(()) => {
                self.state
                    .log_message(format!("$ {command} → ok, awaiting poll confirmation"));
                self.refresh_pane(plan.target.pane);
                self.state.invalidate_inspect(&plan.target);
            }
            Err(e) => {
                if matches!(plan.kind, ActionKind::Stop | ActionKind::Kill)
                    || (plan.kind == ActionKind::Restart && completed_steps > 0)
                {
                    self.expire_stop_attribution();
                    self.stop_attribution.insert(
                        plan.target.name.clone(),
                        StopAttribution {
                            completion_floor: self.poll_sequence,
                            completed_at: Instant::now(),
                        },
                    );
                }
                let failed_command = plan
                    .commands
                    .get(completed_steps)
                    .map(|args| format!("container {}", args.join(" ")))
                    .unwrap_or(command);
                self.state.log_message(format!(
                    "$ {failed_command} → failed (step {}/{})\n{}",
                    completed_steps + 1,
                    plan.commands.len(),
                    e.raw()
                ));
                if plan.kind == ActionKind::Restart {
                    let step = if completed_steps > 0 {
                        "stopped, but start failed"
                    } else {
                        "stop failed"
                    };
                    self.state.toast(
                        format!("restart {}: {step}: {}", plan.target.name, e.gist()),
                        true,
                    );
                    self.sync_follower();
                    return;
                }
                match e {
                    CliError::NotFound { .. } => {
                        self.state
                            .toast(format!("{}: already gone", plan.target.name), false);
                        self.refresh_pane(plan.target.pane);
                    }
                    other => self.state.toast(other.gist(), true),
                }
            }
        }
        self.sync_follower();
    }

    pub fn dispatch(&mut self, cmd: Command) {
        if self.state.quitting
            && !matches!(
                cmd,
                Command::Quit
                    | Command::ForceQuit
                    | Command::CloseOverlay
                    | Command::SetQuitScroll(_)
            )
        {
            return;
        }
        match cmd {
            Command::Quit => {
                let commands = self.quit_blocked_by();
                if commands.is_empty() {
                    self.state.quit = true;
                } else {
                    self.confirmation = None;
                    self.tag_source = None;
                    self.state.overlay = Overlay::QuitConfirm {
                        commands,
                        scroll: 0,
                    };
                }
            }
            Command::ForceQuit => self.state.quit = true,
            Command::WaitAndQuit => {
                if matches!(self.state.overlay, Overlay::QuitConfirm { .. }) {
                    self.state.quitting = true;
                    self.refresh_quit();
                }
            }
            Command::SetQuitScroll(to) => {
                if let Overlay::QuitConfirm { scroll, .. } = &mut self.state.overlay {
                    *scroll = to;
                }
            }
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
            Command::OpenSettings => self.state.overlay = Overlay::Settings { cursor: 0 },
            Command::SettingsMove(delta) => {
                if let Overlay::Settings { cursor } = &mut self.state.overlay {
                    let last = Setting::ALL.len() as isize - 1;
                    *cursor = (*cursor as isize + delta).clamp(0, last) as usize;
                }
            }
            Command::SettingsToggle => {
                if let Overlay::Settings { cursor } = self.state.overlay {
                    if let Some(setting) = Setting::ALL.get(cursor) {
                        setting.cycle(&mut self.state.config);
                        setting.apply(&self.state.config, &mut self.state.persisted);
                        match self.state.persisted.save() {
                            Ok(_) => {}
                            Err(e) => self
                                .state
                                .toast(format!("could not save config: {e}"), true),
                        }
                    }
                }
            }
            Command::OpenMessageLog => self.state.overlay = Overlay::MessageLog,
            Command::CloseOverlay => {
                self.state.quitting = false;
                self.confirmation = None;
                self.tag_source = None;
                self.state.overlay = Overlay::None;
            }
            Command::DismissBanner => self.state.version_banner = None,
            Command::Run(action) => self.run_ui_action(action),
            Command::ConfirmYes => {
                if matches!(self.state.overlay, Overlay::Confirm { .. }) {
                    self.state.overlay = Overlay::None;
                    if let Some(plan) = self.confirmation.take() {
                        self.run_confirmed(plan);
                    }
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
                Overlay::PullInput { text }
                | Overlay::TagInput { text }
                | Overlay::CreateVolumeInput { text } => text.push(c),
                _ => {}
            },
            Command::OverlayBackspace => match &mut self.state.overlay {
                Overlay::PullInput { text }
                | Overlay::TagInput { text }
                | Overlay::CreateVolumeInput { text } => {
                    text.pop();
                }
                _ => {}
            },
            Command::OverlaySubmit => match &self.state.overlay {
                Overlay::PullInput { text } => {
                    let reference = text.trim().to_string();
                    self.state.overlay = Overlay::None;
                    if !reference.is_empty() {
                        self.start_pull(reference);
                    }
                }
                Overlay::TagInput { text } => {
                    let dest = text.trim().to_string();
                    if dest.is_empty() {
                        self.state.toast("enter a new reference", true);
                    } else {
                        self.submit_tag(dest);
                    }
                }
                Overlay::CreateVolumeInput { text } => {
                    let name = text.trim().to_string();
                    self.state.overlay = Overlay::None;
                    if !name.is_empty() {
                        self.open_confirm(
                            ActionKind::CreateVolume,
                            name.clone(),
                            Client::<R>::create_volume_args(&name),
                        );
                    }
                }
                _ => {}
            },
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
                    self.state.follow = false;
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
        match pane {
            Pane::Images => self.images_dirty = true,
            Pane::Volumes => self.volumes_dirty = true,
            Pane::Networks => self.networks_dirty = true,
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
                let commands = match kind {
                    ActionKind::Restart => {
                        vec![Client::<R>::stop_args(&id), Client::<R>::start_args(&id)]
                    }
                    ActionKind::Stop => vec![Client::<R>::stop_args(&id)],
                    _ => vec![Client::<R>::start_args(&id)],
                };
                self.run_confirmed(ActionPlan {
                    kind,
                    target: Target::new(Pane::Containers, id),
                    commands,
                    tag: None,
                });
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
            (Pane::Images, UiAction::Tag) => {
                let Some(image) = self.state.selected_image() else {
                    return;
                };
                self.tag_source = Some((image.reference.clone(), image.digest.clone()));
                self.state.overlay = Overlay::TagInput {
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
            (Pane::Volumes, UiAction::Create) => {
                self.state.overlay = Overlay::CreateVolumeInput {
                    text: String::new(),
                };
            }
            (Pane::Volumes, UiAction::Delete) => {
                let Some(v) = self.state.selected_volume() else {
                    return;
                };
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

    fn submit_tag(&mut self, dest: String) {
        let Some((source, digest)) = self.tag_source.clone() else {
            self.state.overlay = Overlay::None;
            return;
        };
        self.present_confirmation(ActionPlan {
            kind: ActionKind::TagImage,
            target: Target::new(Pane::Images, &source),
            commands: vec![Client::<R>::tag_image_args(&source, &dest)],
            tag: Some(TagTarget {
                reference: dest,
                digest,
            }),
        });
    }

    fn open_confirm(&mut self, action: ActionKind, target: String, args: Vec<String>) {
        let pane = match action {
            ActionKind::DeleteImage | ActionKind::TagImage | ActionKind::PruneImages => {
                Pane::Images
            }
            ActionKind::DeleteVolume | ActionKind::CreateVolume | ActionKind::PruneVolumes => {
                Pane::Volumes
            }
            _ => Pane::Containers,
        };
        self.present_confirmation(ActionPlan {
            kind: action,
            target: Target::new(pane, target),
            commands: vec![args],
            tag: None,
        });
    }

    fn present_confirmation(&mut self, plan: ActionPlan) {
        if let Err(reason) = self.check_action(&plan) {
            self.state.toast(reason, true);
            return;
        }
        self.state.overlay = Overlay::Confirm {
            command: plan.command(),
            action: plan.kind,
            target: plan.target.name.clone(),
        };
        self.confirmation = Some(plan);
    }

    fn is_prune(kind: ActionKind) -> bool {
        matches!(
            kind,
            ActionKind::PruneContainers | ActionKind::PruneImages | ActionKind::PruneVolumes
        )
    }

    fn check_action(&self, plan: &ActionPlan) -> Result<(), String> {
        let pane = plan.target.pane;
        let name = &plan.target.name;
        if Self::is_prune(plan.kind) {
            if self.prune.is_some() {
                return Err("a prune is already running".into());
            }
            if self.pending.has_kind(pane) {
                return Err(format!("{}: actions already pending", pane.title()));
            }
            if pane == Pane::Images && self.state.pull.is_some() {
                return Err("a pull is already running".into());
            }
            return Ok(());
        }
        if self.prune.is_some_and(|(_, active)| active == pane) {
            return Err(format!("{}: prune already running", pane.title()));
        }
        self.pending
            .can_begin(plan)
            .map_err(|target| format!("{}: action already pending", target.name))?;
        if let Some(pull) = &self.state.pull {
            if plan.targets().iter().any(|target| {
                target.pane == Pane::Images
                    && client::canonical_reference(&target.name)
                        == client::canonical_reference(&pull.reference)
            }) {
                return Err(format!("{}: pull already running", pull.reference));
            }
        }
        match pane {
            Pane::Containers => {
                let c = self
                    .state
                    .containers
                    .iter()
                    .find(|c| c.id == *name)
                    .ok_or_else(|| format!("{name}: already gone"))?;
                if matches!(plan.kind, ActionKind::Stop | ActionKind::Kill) && !c.is_running() {
                    return Err(format!("{name}: no longer running"));
                }
                if plan.kind == ActionKind::Start && c.is_running() {
                    return Err(format!("{name}: already running"));
                }
            }
            Pane::Images => {
                let image = self
                    .state
                    .images
                    .iter()
                    .find(|i| i.reference == *name)
                    .ok_or_else(|| format!("{name}: already gone"))?;
                if let Some(tag) = &plan.tag {
                    if image.digest != tag.digest {
                        return Err(format!("{name}: image changed; preview the action again"));
                    }
                }
            }
            Pane::Volumes if plan.kind != ActionKind::CreateVolume => {
                let volume = self
                    .state
                    .volumes
                    .iter()
                    .find(|v| v.name == *name)
                    .ok_or_else(|| format!("{name}: already gone"))?;
                if volume.in_use() {
                    return Err(format!(
                        "cannot delete {name}: in use by {}",
                        volume.in_use_by.join(", ")
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn run_confirmed(&mut self, plan: ActionPlan) {
        if let Err(reason) = self.check_action(&plan) {
            self.state.toast(reason, true);
            return;
        }
        if Self::is_prune(plan.kind) {
            self.prune_generation += 1;
            let generation = self.prune_generation;
            self.prune = Some((generation, plan.target.pane));
            let command = plan.command();
            self.prune_command = Some(command.clone());
            self.state.activity = Some(Activity {
                label: command.clone(),
                started: Instant::now(),
            });
            self.state.log_message(format!("$ {command}"));
            let client = self.client.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let result = client.run_action(&plan.commands[0]).await.map(|_| ());
                let _ = tx
                    .send(AppEvent::PruneDone {
                        generation,
                        command,
                        result,
                    })
                    .await;
            });
            return;
        }
        let Ok(id) = self.pending.begin(plan.clone()) else {
            return;
        };
        if plan.kind == ActionKind::CreateVolume {
            self.state.insert_creating_volume(&plan.target.name);
        }
        self.sync_pending();
        self.spawn_plan(id, plan);
    }

    fn start_pull(&mut self, reference: String) {
        let last_segment = reference.rsplit('/').next().unwrap_or(&reference);
        let reference = if last_segment.contains(':') {
            reference
        } else {
            format!("{reference}:latest")
        };
        if self.prune.is_some_and(|(_, pane)| pane == Pane::Images) {
            self.state.toast("images: prune already running", true);
            return;
        }
        if self
            .pending
            .pending_for(&Target::new(Pane::Images, &reference))
            .is_some()
        {
            self.state
                .toast(format!("{reference}: action already pending"), true);
            return;
        }
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

    pub fn prepare_exec(&mut self) -> Vec<String> {
        let id = self.state.exec_request.take().unwrap_or_default();
        self.stop_follower();
        self.cancel_backlog();
        self.cancel_inspect();
        self.follow_generation += 1;
        self.state.log_owner = None;
        Client::<R>::exec_shell_args(&id)
    }

    pub fn after_exec(&mut self) {
        self.spawn_containers_poll();
        self.sync_follower();
        self.ensure_inspect();
    }

    /// Exact commands whose completion has not yet reached the Engine.
    pub fn quit_blocked_by(&self) -> Vec<String> {
        let mut commands: Vec<_> = self
            .pending
            .in_flight()
            .iter()
            .map(|plan| plan.command())
            .collect();
        if let Some(command) = &self.prune_command {
            commands.push(command.clone());
        }
        if let Some(pull) = &self.state.pull {
            commands.push(format!(
                "container {}",
                Client::<R>::pull_args(&pull.reference).join(" ")
            ));
        }
        if self.state.service_starting {
            commands.push(format!(
                "container {}",
                Client::<R>::system_start_args().join(" ")
            ));
        }
        commands
    }

    fn refresh_quit(&mut self) {
        if !self.state.quitting && !matches!(self.state.overlay, Overlay::QuitConfirm { .. }) {
            return;
        }
        let commands = self.quit_blocked_by();
        if commands.is_empty() {
            self.state.overlay = Overlay::None;
            if self.state.quitting {
                self.state.quit = true;
            }
        } else if let Overlay::QuitConfirm {
            commands: displayed,
            ..
        } = &mut self.state.overlay
        {
            *displayed = commands;
        }
    }

    pub fn shutdown(&mut self) -> Vec<String> {
        let interrupted = self.quit_blocked_by();
        self.stop_follower();
        self.cancel_backlog();
        self.cancel_inspect();
        self.follow_generation += 1;
        if let Some(kill) = self.pull_kill.take() {
            kill.kill();
        }
        if let Some(kill) = self.service_kill.take() {
            kill.kill();
        }
        interrupted
    }

    pub fn follower_id(&self) -> Option<&str> {
        self.follower.as_ref().map(|(id, _)| id.as_str())
    }
}

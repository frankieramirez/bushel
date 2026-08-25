//! `AppState`: the single state tree the UI renders from. Mutated only by the
//! Engine's update loop; the UI never writes to it.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use crate::client::model::{ContainerJson, ImageJson, StatsJson, VolumeJson};

/// Ring-buffer caps.
pub const LOG_RING_CAP: usize = 10_000;
pub const MESSAGE_LOG_CAP: usize = 1_000;
/// 5 minutes of telemetry at the 1s poll cadence; one sample per column.
pub const TELEMETRY_HISTORY: usize = 300;
/// Poll ticks a finished action may wait for state confirmation.
pub const CONFIRM_TICKS: u8 = 2;
/// Consecutive containers-poll parse failures before the degraded banner.
pub const DEGRADED_THRESHOLD: u32 = 3;
/// The very first launch holds the splash for this long — the one deliberate
/// exception to "the splash never adds latency". Any key still skips.
pub const FIRST_RUN_DWELL: std::time::Duration = std::time::Duration::from_millis(1000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Splash,
    Main,
    ServiceDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Containers,
    Images,
    Volumes,
}

impl Pane {
    pub fn index(self) -> usize {
        match self {
            Pane::Containers => 0,
            Pane::Images => 1,
            Pane::Volumes => 2,
        }
    }

    pub fn next(self) -> Pane {
        match self {
            Pane::Containers => Pane::Images,
            Pane::Images => Pane::Volumes,
            Pane::Volumes => Pane::Containers,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Pane::Containers => "containers",
            Pane::Images => "images",
            Pane::Volumes => "volumes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Logs,
    Inspect,
}

/// High-level user actions, resolved against the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    Start,
    Stop,
    Restart,
    Kill,
    Delete,
    Prune,
    Exec,
    Pull,
    LogsTab,
    InspectTab,
}

/// What a confirmed / running action is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Start,
    Stop,
    Kill,
    Restart,
    DeleteContainer,
    PruneContainers,
    DeleteImage,
    PruneImages,
    DeleteVolume,
    PruneVolumes,
}

impl ActionKind {
    /// The container state a poll must show for the action to be confirmed.
    /// `None` means confirmation is "the row disappeared" or not state-based.
    pub fn expected_state(self) -> Option<&'static str> {
        match self {
            ActionKind::Start | ActionKind::Restart => Some("running"),
            ActionKind::Stop | ActionKind::Kill => Some("stopped"),
            _ => None,
        }
    }

    pub fn past_tense(self) -> &'static str {
        match self {
            ActionKind::Start => "started",
            ActionKind::Stop => "stopped",
            ActionKind::Kill => "killed",
            ActionKind::Restart => "restarted",
            ActionKind::DeleteContainer | ActionKind::DeleteImage | ActionKind::DeleteVolume => {
                "deleted"
            }
            ActionKind::PruneContainers | ActionKind::PruneImages | ActionKind::PruneVolumes => {
                "pruned"
            }
        }
    }
}

/// An in-flight action on one entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingPhase {
    /// Subprocess still running (mutations have no deadline).
    InFlight,
    /// Subprocess exited 0; waiting for a poll to confirm, at most N more ticks.
    Confirming(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    pub kind: ActionKind,
    pub phase: PendingPhase,
}

#[derive(Debug, Clone)]
pub struct ContainerEntry {
    pub id: String,
    pub image: String,
    pub state: String,
    pub created: Option<String>,
    pub cpus: Option<u32>,
    pub volumes: Vec<String>,
    pub cpu_percent: Option<f64>,
    pub mem_bytes: Option<u64>,
    /// Newest-first derived samples for the strip. Empty until the second stats poll.
    pub telemetry: VecDeque<TelemetrySample>,
    pub pending: Option<Pending>,
}

/// One derived stats sample. Rates are first-differences of cumulative counters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TelemetrySample {
    pub cpu: Option<f64>,
    pub mem: Option<f64>,
    pub rx: Option<u64>,
    pub tx: Option<u64>,
    pub r: Option<u64>,
    pub w: Option<u64>,
}

/// The previous stats poll's raw counters, used to first-difference the next tick.
#[derive(Debug, Clone, Copy)]
pub struct StatsSnapshot {
    pub at: Instant,
    pub cpu_usage_usec: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
}

impl ContainerEntry {
    pub fn is_running(&self) -> bool {
        self.state == "running"
    }
}

#[derive(Debug, Clone)]
pub struct ImageEntry {
    pub reference: String,
    pub size: Option<u64>,
    pub created: Option<String>,
    pub pending: Option<Pending>,
}

#[derive(Debug, Clone)]
pub struct VolumeEntry {
    pub name: String,
    pub in_use_by: Vec<String>,
    pub created: Option<String>,
    pub pending: Option<Pending>,
}

impl VolumeEntry {
    pub fn in_use(&self) -> bool {
        !self.in_use_by.is_empty()
    }
}

/// Modal overlays; at most one at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    None,
    ActionMenu,
    /// Command preview + the action it would run on confirmation.
    Confirm {
        command: String,
        action: ActionKind,
        target: String,
    },
    Help,
    MessageLog,
    PullInput {
        text: String,
    },
}

/// A menu row in the action menu / a direct-key binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionItem {
    pub key: char,
    pub label: &'static str,
    pub destructive: bool,
    pub action: UiAction,
}

const fn item(key: char, label: &'static str, destructive: bool, action: UiAction) -> ActionItem {
    ActionItem {
        key,
        label,
        destructive,
        action,
    }
}

/// Streaming pull shown in the detail pane.
#[derive(Debug, Clone)]
pub struct PullState {
    pub reference: String,
    pub lines: Vec<String>,
    pub started: Instant,
}

/// Bottom-bar activity for long mutations without per-row pending (prune).
#[derive(Debug, Clone)]
pub struct Activity {
    pub label: String,
    pub started: Instant,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

pub struct AppState {
    pub screen: Screen,
    pub pane: Pane,
    pub focus: Focus,
    pub zoom: bool,
    pub detail_tab: DetailTab,
    pub overlay: Overlay,
    pub quit: bool,

    pub containers: Vec<ContainerEntry>,
    pub images: Vec<ImageEntry>,
    pub volumes: Vec<VolumeEntry>,
    /// Selected entity id per pane; re-anchored across polls.
    pub selected: [Option<String>; 3],

    pub filter: String,
    pub filter_input: bool,

    pub detail_scroll: u16,
    /// Auto-scroll the logs tab to the newest line.
    pub follow: bool,
    /// Ring buffer of log lines for the followed container.
    pub log_lines: Vec<String>,
    /// Which container the log buffer belongs to.
    pub log_owner: Option<String>,
    /// Backlog request still in flight; follow lines buffer until it lands.
    pub logs_loading: bool,
    pub follow_ended: bool,

    /// Raw inspect JSON per entity id.
    pub inspect_cache: HashMap<String, String>,
    /// Inspect fetch in flight for this id.
    pub inspect_loading: Option<String>,

    pub messages: Vec<String>,
    pub toast: Option<Toast>,

    pub pull: Option<PullState>,
    pub activity: Option<Activity>,

    pub cli_version: Option<String>,
    pub version_banner: Option<String>,
    pub degraded: bool,
    pub parse_failures: u32,

    pub service_output: Vec<String>,
    pub service_starting: bool,

    /// True once the first containers poll has landed (splash may dissolve).
    pub first_data: bool,
    /// When the app came up; the splash only becomes visible if the startup
    /// probes are still running after a grace period (no sub-100ms flash).
    pub started_at: Instant,
    /// Very first launch: the splash shows immediately and dwells FIRST_RUN_DWELL.
    pub first_run: bool,
    pub tick: u64,
    pub last_poll_at: Option<Instant>,
    /// Exec request the outer loop must service (suspend TUI, run, restore).
    pub exec_request: Option<String>,
    /// Outcomes a poll confirmed since the engine last announced them.
    confirmations: Vec<(String, ActionKind)>,
}

impl AppState {
    pub fn new(no_splash: bool) -> Self {
        Self {
            screen: if no_splash {
                Screen::Main
            } else {
                Screen::Splash
            },
            pane: Pane::Containers,
            focus: Focus::List,
            zoom: false,
            detail_tab: DetailTab::Logs,
            overlay: Overlay::None,
            quit: false,
            containers: Vec::new(),
            images: Vec::new(),
            volumes: Vec::new(),
            selected: [None, None, None],
            filter: String::new(),
            filter_input: false,
            detail_scroll: 0,
            follow: true,
            log_lines: Vec::new(),
            log_owner: None,
            logs_loading: false,
            follow_ended: false,
            inspect_cache: HashMap::new(),
            inspect_loading: None,
            messages: Vec::new(),
            toast: None,
            pull: None,
            activity: None,
            cli_version: None,
            version_banner: None,
            degraded: false,
            parse_failures: 0,
            service_output: Vec::new(),
            service_starting: false,
            first_data: false,
            started_at: Instant::now(),
            first_run: false,
            tick: 0,
            last_poll_at: None,
            exec_request: None,
            confirmations: Vec::new(),
        }
    }

    /// May the splash dissolve into the layout? Data must have arrived, and on
    /// the very first launch the dwell must also have elapsed.
    pub fn splash_may_dissolve(&self) -> bool {
        self.first_data && (!self.first_run || self.started_at.elapsed() >= FIRST_RUN_DWELL)
    }

    // ---- messages -------------------------------------------------------

    pub fn log_message(&mut self, msg: impl Into<String>) {
        self.messages.push(msg.into());
        if self.messages.len() > MESSAGE_LOG_CAP {
            let excess = self.messages.len() - MESSAGE_LOG_CAP;
            self.messages.drain(..excess);
        }
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool) {
        let text = text.into();
        self.log_message(text.clone());
        self.toast = Some(Toast {
            text,
            error,
            at: Instant::now(),
        });
    }

    // ---- filtering & selection ------------------------------------------

    /// Fuzzy subsequence match, case-insensitive.
    pub fn fuzzy_match(needle: &str, hay: &str) -> bool {
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

    /// Indexes (into the entity vec) of rows matching the filter, display order.
    /// Containers are stored pre-sorted (running first, then name).
    pub fn visible_rows(&self) -> Vec<usize> {
        let f = &self.filter;
        match self.pane {
            Pane::Containers => self
                .containers
                .iter()
                .enumerate()
                .filter(|(_, c)| Self::fuzzy_match(f, &format!("{} {} {}", c.id, c.image, c.state)))
                .map(|(i, _)| i)
                .collect(),
            Pane::Images => self
                .images
                .iter()
                .enumerate()
                .filter(|(_, i)| Self::fuzzy_match(f, &i.reference))
                .map(|(i, _)| i)
                .collect(),
            Pane::Volumes => self
                .volumes
                .iter()
                .enumerate()
                .filter(|(_, v)| Self::fuzzy_match(f, &v.name))
                .map(|(i, _)| i)
                .collect(),
        }
    }

    fn entity_id(&self, pane: Pane, idx: usize) -> Option<String> {
        match pane {
            Pane::Containers => self.containers.get(idx).map(|c| c.id.clone()),
            Pane::Images => self.images.get(idx).map(|i| i.reference.clone()),
            Pane::Volumes => self.volumes.get(idx).map(|v| v.name.clone()),
        }
    }

    /// Position of the selection within `visible_rows()`, if any.
    pub fn selected_pos(&self) -> Option<usize> {
        let want = self.selected[self.pane.index()].as_deref()?;
        self.visible_rows()
            .iter()
            .position(|&i| self.entity_id(self.pane, i).as_deref() == Some(want))
    }

    /// Index into the entity vec of the current selection.
    pub fn selected_row(&self) -> Option<usize> {
        let rows = self.visible_rows();
        let pos = self.selected_pos()?;
        rows.get(pos).copied()
    }

    pub fn selected_container(&self) -> Option<&ContainerEntry> {
        if self.pane != Pane::Containers {
            return None;
        }
        self.selected_row().and_then(|i| self.containers.get(i))
    }

    pub fn selected_image(&self) -> Option<&ImageEntry> {
        if self.pane != Pane::Images {
            return None;
        }
        self.selected_row().and_then(|i| self.images.get(i))
    }

    pub fn selected_volume(&self) -> Option<&VolumeEntry> {
        if self.pane != Pane::Volumes {
            return None;
        }
        self.selected_row().and_then(|i| self.volumes.get(i))
    }

    /// Move the selection by `delta` within visible rows, clamped.
    pub fn move_selection(&mut self, delta: isize) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            self.selected[self.pane.index()] = None;
            return;
        }
        let pos = self.selected_pos().unwrap_or(0) as isize;
        let next = (pos + delta).clamp(0, rows.len() as isize - 1) as usize;
        self.selected[self.pane.index()] = self.entity_id(self.pane, rows[next]);
    }

    pub fn select_edge(&mut self, top: bool) {
        let rows = self.visible_rows();
        let idx = if top { rows.first() } else { rows.last() };
        self.selected[self.pane.index()] = idx.and_then(|&i| self.entity_id(self.pane, i));
    }

    /// Re-anchor (or initialize) the selection after a list update.
    pub fn clamp_selection(&mut self) {
        for pane in [Pane::Containers, Pane::Images, Pane::Volumes] {
            let exists = |id: &str| match pane {
                Pane::Containers => self.containers.iter().any(|c| c.id == id),
                Pane::Images => self.images.iter().any(|i| i.reference == id),
                Pane::Volumes => self.volumes.iter().any(|v| v.name == id),
            };
            let sel = &mut self.selected[pane.index()];
            if let Some(id) = sel {
                if !exists(id) {
                    *sel = None;
                }
            }
            if sel.is_none() {
                *self.selected.get_mut(pane.index()).unwrap() = match pane {
                    Pane::Containers => self.containers.first().map(|c| c.id.clone()),
                    Pane::Images => self.images.first().map(|i| i.reference.clone()),
                    Pane::Volumes => self.volumes.first().map(|v| v.name.clone()),
                };
            }
        }
    }

    // ---- list updates -----------------------------------------------------

    /// Replace the containers list from a poll, preserving pending markers and
    /// stats, sorting running-first then alphabetical. Returns message-log diffs
    /// and the ids of externally stopped containers.
    pub fn update_containers(&mut self, fresh: &[ContainerJson]) -> (Vec<String>, Vec<String>) {
        let mut diffs = Vec::new();
        let mut external_stops = Vec::new();

        let mut next: Vec<ContainerEntry> = fresh
            .iter()
            .map(|c| ContainerEntry {
                id: c.id.clone(),
                image: c.image_reference().to_string(),
                state: c.status.state.clone(),
                created: c.configuration.creation_date.clone(),
                cpus: c.configuration.resources.as_ref().and_then(|r| r.cpus),
                volumes: c.volume_sources().map(|s| s.to_string()).collect(),
                cpu_percent: None,
                mem_bytes: None,
                telemetry: VecDeque::new(),
                pending: None,
            })
            .collect();
        next.sort_by(|a, b| (!a.is_running(), &a.id).cmp(&(!b.is_running(), &b.id)));

        for entry in &mut next {
            if let Some(old) = self.containers.iter().find(|o| o.id == entry.id) {
                entry.cpu_percent = old.cpu_percent;
                entry.mem_bytes = old.mem_bytes;
                entry.telemetry = old.telemetry.clone();
                entry.pending = old.pending;
                if old.state != entry.state {
                    diffs.push(format!("{}: {} → {}", entry.id, old.state, entry.state));
                    // state changed → cached inspect is stale
                    self.inspect_cache.remove(&entry.id);
                    let ours = matches!(
                        old.pending.map(|p| p.kind),
                        Some(
                            ActionKind::Stop
                                | ActionKind::Kill
                                | ActionKind::Restart
                                | ActionKind::DeleteContainer
                        )
                    );
                    if old.state == "running" && !entry.is_running() && !ours {
                        external_stops.push(entry.id.clone());
                    }
                }
                if !entry.is_running() {
                    entry.cpu_percent = None;
                    entry.mem_bytes = None;
                }
            } else {
                diffs.push(format!("{}: appeared ({})", entry.id, entry.state));
            }
        }
        for old in &self.containers {
            if !next.iter().any(|n| n.id == old.id) {
                diffs.push(format!("{}: removed", old.id));
                self.inspect_cache.remove(&old.id);
                if let Some(p) = old.pending {
                    // a pending delete confirms by the row disappearing
                    self.confirmations.push((old.id.clone(), p.kind));
                }
            }
        }

        self.containers = next;
        self.confirm_pending();
        self.clamp_selection();
        self.first_data = true;
        (diffs, external_stops)
    }

    pub fn update_images(&mut self, fresh: &[ImageJson]) {
        let mut next: Vec<ImageEntry> = fresh
            .iter()
            .map(|i| ImageEntry {
                reference: i.reference().to_string(),
                size: i.display_size(),
                created: i.configuration.creation_date.clone(),
                pending: None,
            })
            .collect();
        next.sort_by(|a, b| a.reference.cmp(&b.reference));
        for entry in &mut next {
            if let Some(old) = self.images.iter().find(|o| o.reference == entry.reference) {
                entry.pending = old.pending;
            }
        }
        for old in &self.images {
            if let Some(p) = old.pending {
                if !next.iter().any(|n| n.reference == old.reference) {
                    self.confirmations.push((old.reference.clone(), p.kind));
                }
            }
        }
        self.images = next;
        self.confirm_pending();
        self.clamp_selection();
    }

    pub fn update_volumes(&mut self, fresh: &[VolumeJson]) {
        let mut next: Vec<VolumeEntry> = fresh
            .iter()
            .map(|v| VolumeEntry {
                name: v.name().to_string(),
                in_use_by: Vec::new(),
                created: v.configuration.creation_date.clone(),
                pending: None,
            })
            .collect();
        next.sort_by(|a, b| a.name.cmp(&b.name));
        for entry in &mut next {
            if let Some(old) = self.volumes.iter().find(|o| o.name == entry.name) {
                entry.pending = old.pending;
            }
        }
        for old in &self.volumes {
            if let Some(p) = old.pending {
                if !next.iter().any(|n| n.name == old.name) {
                    self.confirmations.push((old.name.clone(), p.kind));
                }
            }
        }
        self.volumes = next;
        self.recompute_in_use();
        self.confirm_pending();
        self.clamp_selection();
    }

    /// In-use badges: a volume is in use when any container references it.
    pub fn recompute_in_use(&mut self) {
        for v in &mut self.volumes {
            v.in_use_by = self
                .containers
                .iter()
                .filter(|c| c.volumes.iter().any(|s| s == &v.name))
                .map(|c| c.id.clone())
                .collect();
        }
    }

    /// Apply a stats sample: mem directly; CPU% and byte rates from consecutive
    /// cumulative counters. The first sample for an id is swallowed (baseline only).
    pub fn apply_stats(
        &mut self,
        stats: &[StatsJson],
        prev: &HashMap<String, StatsSnapshot>,
        now: Instant,
    ) -> HashMap<String, StatsSnapshot> {
        let mut next = HashMap::new();
        for s in stats {
            let snap = StatsSnapshot {
                at: now,
                cpu_usage_usec: s.cpu_usage_usec,
                network_rx_bytes: s.network_rx_bytes,
                network_tx_bytes: s.network_tx_bytes,
                block_read_bytes: s.block_read_bytes,
                block_write_bytes: s.block_write_bytes,
            };
            next.insert(s.id.clone(), snap);
            let Some(c) = self.containers.iter_mut().find(|c| c.id == s.id) else {
                continue;
            };
            c.mem_bytes = Some(s.memory_usage_bytes);
            let Some(prev) = prev.get(&s.id) else {
                continue; // swallow the first sample
            };
            let elapsed = now.duration_since(prev.at);
            if elapsed.is_zero() {
                continue;
            }
            let wall_usec = elapsed.as_micros() as f64;
            let elapsed_secs = elapsed.as_secs_f64();

            let cpu = if wall_usec > 0.0 && s.cpu_usage_usec >= prev.cpu_usage_usec {
                let delta = (s.cpu_usage_usec - prev.cpu_usage_usec) as f64;
                Some((delta / wall_usec * 100.0).min(999.0))
            } else {
                None
            };
            if let Some(pct) = cpu {
                c.cpu_percent = Some(pct);
            }

            let mem = (s.memory_limit_bytes > 0)
                .then(|| s.memory_usage_bytes as f64 / s.memory_limit_bytes as f64 * 100.0);

            let rate = |cur: u64, old: u64| {
                (cur >= old && elapsed_secs > 0.0)
                    .then(|| ((cur - old) as f64 / elapsed_secs).round() as u64)
            };

            c.telemetry.push_front(TelemetrySample {
                cpu,
                mem,
                rx: rate(s.network_rx_bytes, prev.network_rx_bytes),
                tx: rate(s.network_tx_bytes, prev.network_tx_bytes),
                r: rate(s.block_read_bytes, prev.block_read_bytes),
                w: rate(s.block_write_bytes, prev.block_write_bytes),
            });
            while c.telemetry.len() > TELEMETRY_HISTORY {
                c.telemetry.pop_back();
            }
        }
        next
    }

    // ---- pending actions ----------------------------------------------------

    pub fn pending_of(&self, id: &str) -> Option<Pending> {
        self.containers
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.pending)
            .or_else(|| {
                self.images
                    .iter()
                    .find(|i| i.reference == id)
                    .and_then(|i| i.pending)
            })
            .or_else(|| {
                self.volumes
                    .iter()
                    .find(|v| v.name == id)
                    .and_then(|v| v.pending)
            })
    }

    pub fn set_pending(&mut self, id: &str, pending: Option<Pending>) {
        if let Some(c) = self.containers.iter_mut().find(|c| c.id == id) {
            c.pending = pending;
        } else if let Some(i) = self.images.iter_mut().find(|i| i.reference == id) {
            i.pending = pending;
        } else if let Some(v) = self.volumes.iter_mut().find(|v| v.name == id) {
            v.pending = pending;
        }
    }

    /// After a poll: clear pendings whose expected state is confirmed, and count
    /// down the confirmation cap for the rest.
    fn confirm_pending(&mut self) {
        let mut confirmed_now = Vec::new();
        for c in &mut self.containers {
            let Some(p) = c.pending else { continue };
            let PendingPhase::Confirming(ticks) = p.phase else {
                continue;
            };
            let confirmed = p.kind.expected_state().is_some_and(|s| c.state == s);
            if confirmed {
                confirmed_now.push((c.id.clone(), p.kind));
            }
            if confirmed || ticks <= 1 {
                c.pending = None;
            } else {
                c.pending = Some(Pending {
                    kind: p.kind,
                    phase: PendingPhase::Confirming(ticks - 1),
                });
            }
        }
        self.confirmations.extend(confirmed_now);
        // Images and volumes only confirm by disappearance (delete): rows still
        // present count down.
        for i in &mut self.images {
            if let Some(Pending {
                kind,
                phase: PendingPhase::Confirming(t),
            }) = i.pending
            {
                i.pending = (t > 1).then_some(Pending {
                    kind,
                    phase: PendingPhase::Confirming(t - 1),
                });
            }
        }
        for v in &mut self.volumes {
            if let Some(Pending {
                kind,
                phase: PendingPhase::Confirming(t),
            }) = v.pending
            {
                v.pending = (t > 1).then_some(Pending {
                    kind,
                    phase: PendingPhase::Confirming(t - 1),
                });
            }
        }
    }

    /// Poll-confirmed outcomes since the last call (the engine announces these).
    pub fn take_confirmations(&mut self) -> Vec<(String, ActionKind)> {
        std::mem::take(&mut self.confirmations)
    }

    // ---- logs -----------------------------------------------------------------

    pub fn push_log_line(&mut self, line: String) {
        self.log_lines.push(line);
        if self.log_lines.len() > LOG_RING_CAP {
            let excess = self.log_lines.len() - LOG_RING_CAP;
            self.log_lines.drain(..excess);
        }
    }

    // ---- action menu -------------------------------------------------------------

    /// Valid actions for the current selection, in menu order.
    pub fn available_actions(&self) -> Vec<ActionItem> {
        match self.pane {
            Pane::Containers => {
                let Some(c) = self.selected_container() else {
                    return vec![item('P', "prune stopped", true, UiAction::Prune)];
                };
                if c.is_running() {
                    vec![
                        item('s', "stop", false, UiAction::Stop),
                        item('r', "restart", false, UiAction::Restart),
                        item('K', "kill", true, UiAction::Kill),
                        item('d', "delete", true, UiAction::Delete),
                        item('P', "prune stopped", true, UiAction::Prune),
                        item('e', "exec shell", false, UiAction::Exec),
                        item('l', "logs", false, UiAction::LogsTab),
                        item('i', "inspect", false, UiAction::InspectTab),
                    ]
                } else {
                    vec![
                        item('s', "start", false, UiAction::Start),
                        item('d', "delete", true, UiAction::Delete),
                        item('P', "prune stopped", true, UiAction::Prune),
                        item('i', "inspect", false, UiAction::InspectTab),
                    ]
                }
            }
            Pane::Images => {
                let mut items = vec![item('u', "pull by reference", false, UiAction::Pull)];
                if self.selected_image().is_some() {
                    items.push(item('d', "delete", true, UiAction::Delete));
                }
                items.push(item('P', "prune unused", true, UiAction::Prune));
                items
            }
            Pane::Volumes => {
                let mut items = Vec::new();
                if self.selected_volume().is_some() {
                    items.push(item('d', "delete", true, UiAction::Delete));
                }
                items.push(item('P', "prune unreferenced", true, UiAction::Prune));
                items
            }
        }
    }
}

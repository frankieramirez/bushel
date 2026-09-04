use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use crate::client::model::{ContainerJson, ImageJson, NetworkJson, StatsJson, VolumeJson};

pub const LOG_RING_CAP: usize = 10_000;
pub const MESSAGE_LOG_CAP: usize = 1_000;
pub const TELEMETRY_HISTORY: usize = 300;
pub const CONFIRM_TICKS: u8 = 2;
pub const DEGRADED_THRESHOLD: u32 = 3;
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
    Networks,
}

impl Pane {
    pub const COUNT: usize = 4;

    pub fn index(self) -> usize {
        match self {
            Pane::Containers => 0,
            Pane::Images => 1,
            Pane::Volumes => 2,
            Pane::Networks => 3,
        }
    }

    pub fn next(self) -> Pane {
        match self {
            Pane::Containers => Pane::Images,
            Pane::Images => Pane::Volumes,
            Pane::Volumes => Pane::Networks,
            Pane::Networks => Pane::Containers,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Pane::Containers => "containers",
            Pane::Images => "images",
            Pane::Volumes => "volumes",
            Pane::Networks => "networks",
        }
    }

    pub fn key(self) -> char {
        match self {
            Pane::Containers => '1',
            Pane::Images => '2',
            Pane::Volumes => '3',
            Pane::Networks => '4',
        }
    }

    pub const fn all() -> [Pane; Self::COUNT] {
        [
            Pane::Containers,
            Pane::Images,
            Pane::Volumes,
            Pane::Networks,
        ]
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
    Tag,
    LogsTab,
    InspectTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Start,
    Stop,
    Kill,
    Restart,
    DeleteContainer,
    PruneContainers,
    DeleteImage,
    TagImage,
    PruneImages,
    DeleteVolume,
    PruneVolumes,
}

impl ActionKind {
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
            ActionKind::TagImage => "tagged",
            ActionKind::PruneContainers | ActionKind::PruneImages | ActionKind::PruneVolumes => {
                "pruned"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingPhase {
    InFlight,
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
    pub networks: Vec<(String, Option<String>)>,
    pub cpu_percent: Option<f64>,
    pub mem_bytes: Option<u64>,
    pub telemetry: VecDeque<TelemetrySample>,
    pub pending: Option<Pending>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TelemetrySample {
    pub cpu: Option<f64>,
    pub mem: Option<f64>,
    pub rx: Option<u64>,
    pub tx: Option<u64>,
    pub r: Option<u64>,
    pub w: Option<u64>,
}

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

#[derive(Debug, Clone)]
pub struct NetworkEntry {
    pub name: String,
    pub mode: String,
    pub ipv4_subnet: Option<String>,
    pub builtin: bool,
    pub attached: Vec<(String, Option<String>)>,
    pub created: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    None,
    ActionMenu,
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
    TagInput {
        text: String,
    },
}

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

#[derive(Debug, Clone)]
pub struct PullState {
    pub reference: String,
    pub lines: Vec<String>,
    pub started: Instant,
}

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
    pub networks: Vec<NetworkEntry>,
    pub selected: [Option<String>; Pane::COUNT],

    pub filter: String,
    pub filter_input: bool,

    pub detail_scroll: u16,
    pub help_scroll: u16,
    pub follow: bool,
    pub wrap: bool,
    pub log_lines: Vec<String>,
    pub log_owner: Option<String>,
    pub logs_loading: bool,
    pub follow_ended: bool,

    pub inspect_cache: HashMap<String, String>,
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

    pub first_data: bool,
    pub started_at: Instant,
    pub first_run: bool,
    pub tick: u64,
    pub last_poll_at: Option<Instant>,
    pub exec_request: Option<String>,
    pub tag_dest: Option<String>,
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
            help_scroll: 0,
            quit: false,
            containers: Vec::new(),
            images: Vec::new(),
            volumes: Vec::new(),
            networks: Vec::new(),
            selected: [None, None, None, None],
            filter: String::new(),
            filter_input: false,
            detail_scroll: 0,
            follow: true,
            wrap: true,
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
            tag_dest: None,
            confirmations: Vec::new(),
        }
    }

    pub fn splash_may_dissolve(&self) -> bool {
        self.first_data && (!self.first_run || self.started_at.elapsed() >= FIRST_RUN_DWELL)
    }

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

    pub fn visible_rows(&self) -> Vec<usize> {
        self.visible_rows_for(self.pane)
    }

    pub fn visible_rows_for(&self, pane: Pane) -> Vec<usize> {
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
            Pane::Networks => self
                .networks
                .iter()
                .enumerate()
                .filter(|(_, n)| {
                    Self::fuzzy_match(
                        f,
                        &format!(
                            "{} {} {}",
                            n.name,
                            n.mode,
                            n.ipv4_subnet.as_deref().unwrap_or("")
                        ),
                    )
                })
                .map(|(i, _)| i)
                .collect(),
        }
    }

    pub fn pane_len(&self, pane: Pane) -> usize {
        match pane {
            Pane::Containers => self.containers.len(),
            Pane::Images => self.images.len(),
            Pane::Volumes => self.volumes.len(),
            Pane::Networks => self.networks.len(),
        }
    }

    fn entity_id(&self, pane: Pane, idx: usize) -> Option<String> {
        match pane {
            Pane::Containers => self.containers.get(idx).map(|c| c.id.clone()),
            Pane::Images => self.images.get(idx).map(|i| i.reference.clone()),
            Pane::Volumes => self.volumes.get(idx).map(|v| v.name.clone()),
            Pane::Networks => self.networks.get(idx).map(|n| n.name.clone()),
        }
    }

    pub fn selected_pos(&self) -> Option<usize> {
        self.selected_pos_for(self.pane)
    }

    pub fn selected_pos_for(&self, pane: Pane) -> Option<usize> {
        let want = self.selected[pane.index()].as_deref()?;
        self.visible_rows_for(pane)
            .iter()
            .position(|&i| self.entity_id(pane, i).as_deref() == Some(want))
    }

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

    pub fn selected_network(&self) -> Option<&NetworkEntry> {
        if self.pane != Pane::Networks {
            return None;
        }
        self.selected_row().and_then(|i| self.networks.get(i))
    }

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

    pub fn clamp_selection(&mut self) {
        for pane in Pane::all() {
            let exists = |id: &str| match pane {
                Pane::Containers => self.containers.iter().any(|c| c.id == id),
                Pane::Images => self.images.iter().any(|i| i.reference == id),
                Pane::Volumes => self.volumes.iter().any(|v| v.name == id),
                Pane::Networks => self.networks.iter().any(|n| n.name == id),
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
                    Pane::Networks => self.networks.first().map(|n| n.name.clone()),
                };
            }
        }
    }

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
                networks: c
                    .network_attachments()
                    .map(|(name, addr)| (name.to_string(), addr.map(str::to_string)))
                    .collect(),
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
        self.confirm_tag();
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

    pub fn update_networks(&mut self, fresh: &[NetworkJson]) {
        let mut next: Vec<NetworkEntry> = fresh
            .iter()
            .map(|n| NetworkEntry {
                name: n.name().to_string(),
                mode: n.mode().to_string(),
                ipv4_subnet: n.ipv4_subnet().map(str::to_string),
                builtin: n.is_builtin(),
                attached: Vec::new(),
                created: n.configuration.creation_date.clone(),
            })
            .collect();
        next.sort_by(|a, b| a.name.cmp(&b.name));
        self.networks = next;
        self.recompute_network_attachments();
        self.clamp_selection();
    }

    pub fn recompute_in_use(&mut self) {
        for v in &mut self.volumes {
            v.in_use_by = self
                .containers
                .iter()
                .filter(|c| c.volumes.iter().any(|s| s == &v.name))
                .map(|c| c.id.clone())
                .collect();
        }
        self.recompute_network_attachments();
    }

    pub fn recompute_network_attachments(&mut self) {
        for n in &mut self.networks {
            n.attached = self
                .containers
                .iter()
                .flat_map(|c| {
                    c.networks
                        .iter()
                        .filter(|(name, _)| *name == n.name)
                        .map(|(_, addr)| (c.id.clone(), addr.clone()))
                })
                .collect();
        }
    }

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
                continue;
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
        for i in &mut self.images {
            if let Some(Pending {
                kind,
                phase: PendingPhase::Confirming(t),
            }) = i.pending
            {
                if kind == ActionKind::TagImage {
                    continue;
                }
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

    fn confirm_tag(&mut self) {
        let Some(dest) = self.tag_dest.clone() else {
            return;
        };
        let confirming = self.images.iter().any(|i| {
            matches!(
                i.pending,
                Some(Pending {
                    kind: ActionKind::TagImage,
                    phase: PendingPhase::Confirming(_),
                })
            )
        });
        if confirming && self.images.iter().any(|i| i.reference == dest) {
            for i in &mut self.images {
                if i.pending.is_some_and(|p| p.kind == ActionKind::TagImage) {
                    i.pending = None;
                }
            }
            self.confirmations.push((dest, ActionKind::TagImage));
            self.tag_dest = None;
            return;
        }
        for i in &mut self.images {
            if let Some(Pending {
                kind: ActionKind::TagImage,
                phase: PendingPhase::Confirming(t),
            }) = i.pending
            {
                i.pending = (t > 1).then_some(Pending {
                    kind: ActionKind::TagImage,
                    phase: PendingPhase::Confirming(t - 1),
                });
            }
        }
        let still_pending = self
            .images
            .iter()
            .any(|i| i.pending.is_some_and(|p| p.kind == ActionKind::TagImage));
        if !still_pending
            && !matches!(
                self.overlay,
                Overlay::Confirm {
                    action: ActionKind::TagImage,
                    ..
                } | Overlay::TagInput { .. }
            )
        {
            self.tag_dest = None;
        }
    }

    pub fn take_confirmations(&mut self) -> Vec<(String, ActionKind)> {
        std::mem::take(&mut self.confirmations)
    }

    pub fn push_log_line(&mut self, line: String) {
        self.log_lines.push(line);
        if self.log_lines.len() > LOG_RING_CAP {
            let excess = self.log_lines.len() - LOG_RING_CAP;
            self.log_lines.drain(..excess);
        }
    }

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
                    items.push(item('t', "tag", false, UiAction::Tag));
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
            Pane::Networks => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AppState {
        let mut s = AppState::new(true);
        s.containers.extend([
            ContainerEntry {
                id: "qtest".into(),
                image: "alpine:latest".into(),
                state: "running".into(),
                created: None,
                cpus: None,
                volumes: vec![],
                networks: vec![],
                cpu_percent: None,
                mem_bytes: None,
                telemetry: VecDeque::new(),
                pending: None,
            },
            ContainerEntry {
                id: "old-batch".into(),
                image: "alpine:latest".into(),
                state: "stopped".into(),
                created: None,
                cpus: None,
                volumes: vec![],
                networks: vec![],
                cpu_percent: None,
                mem_bytes: None,
                telemetry: VecDeque::new(),
                pending: None,
            },
        ]);
        s.images.extend([
            ImageEntry {
                reference: "alpine:latest".into(),
                size: Some(8),
                created: None,
                pending: None,
            },
            ImageEntry {
                reference: "postgres:16".into(),
                size: Some(16),
                created: None,
                pending: None,
            },
        ]);
        s.volumes.extend([
            VolumeEntry {
                name: "qvol".into(),
                in_use_by: vec![],
                created: None,
                pending: None,
            },
            VolumeEntry {
                name: "scratch".into(),
                in_use_by: vec![],
                created: None,
                pending: None,
            },
        ]);
        s.networks.extend([
            NetworkEntry {
                name: "default".into(),
                mode: "nat".into(),
                ipv4_subnet: Some("192.168.64.0/24".into()),
                builtin: true,
                attached: vec![],
                created: None,
            },
            NetworkEntry {
                name: "foo".into(),
                mode: "nat".into(),
                ipv4_subnet: Some("192.168.65.0/24".into()),
                builtin: false,
                attached: vec![],
                created: None,
            },
        ]);
        s.clamp_selection();
        s
    }

    #[test]
    fn filter_hits_only_the_active_panel() {
        let mut s = sample();
        s.filter = "olbtc".into();
        let containers = s.visible_rows_for(Pane::Containers);
        assert_eq!(containers.len(), 1);
        assert_eq!(s.containers[containers[0]].id, "old-batch");
        assert_eq!(s.visible_rows_for(Pane::Images).len(), 2);
        assert_eq!(s.visible_rows_for(Pane::Volumes).len(), 2);
        assert_eq!(s.visible_rows_for(Pane::Networks).len(), 2);
    }

    #[test]
    fn each_pane_remembers_its_selection() {
        let mut s = sample();
        s.move_selection(1);
        assert_eq!(s.selected[0].as_deref(), Some("old-batch"));
        s.pane = Pane::Images;
        s.move_selection(1);
        assert_eq!(s.selected[1].as_deref(), Some("postgres:16"));
        s.pane = Pane::Containers;
        assert_eq!(s.selected[0].as_deref(), Some("old-batch"));
        s.pane = Pane::Images;
        assert_eq!(s.selected[1].as_deref(), Some("postgres:16"));
        s.pane = Pane::Networks;
        s.move_selection(1);
        assert_eq!(s.selected[3].as_deref(), Some("foo"));
        s.pane = Pane::Containers;
        assert_eq!(s.selected[0].as_deref(), Some("old-batch"));
        s.pane = Pane::Networks;
        assert_eq!(s.selected[3].as_deref(), Some("foo"));
    }

    #[test]
    fn networks_offer_no_mutate_actions() {
        let mut s = sample();
        s.pane = Pane::Networks;
        assert!(s.available_actions().is_empty());
    }

    #[test]
    fn the_detail_tab_jumps_stay_bound_as_direct_keys() {
        let s = sample();
        assert!(s.available_actions().iter().any(|i| i.key == 'l'));
        assert!(s.available_actions().iter().any(|i| i.key == 'i'));
    }

    #[test]
    fn images_pane_offers_tag_on_the_selection() {
        let mut s = sample();
        s.pane = Pane::Images;
        let keys: Vec<char> = s.available_actions().iter().map(|i| i.key).collect();
        assert_eq!(keys, vec!['u', 't', 'd', 'P']);
        assert_eq!(
            s.available_actions()
                .iter()
                .find(|i| i.key == 't')
                .map(|i| i.label),
            Some("tag")
        );
        s.images.clear();
        s.clamp_selection();
        let keys: Vec<char> = s.available_actions().iter().map(|i| i.key).collect();
        assert_eq!(keys, vec!['u', 'P']);
    }
}

//! The one `AppEvent` enum. Every tokio task (poller, action runners, log
//! follower, pull, service start) is a pure producer of these into one mpsc
//! channel; the Engine's update loop is the single writer of `AppState`.

use crate::client::CliError;
use crate::client::model::{ContainerJson, ImageJson, StatsJson, SystemStatusJson, VolumeJson};
use crate::engine::state::{ActionKind, DetailTab, Pane, UiAction};

type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum AppEvent {
    // ---- poll & probe results ----
    Containers(CliResult<Vec<ContainerJson>>),
    Images(CliResult<Vec<ImageJson>>),
    Volumes(CliResult<Vec<VolumeJson>>),
    Stats(CliResult<Vec<StatsJson>>),
    ServiceProbe(CliResult<SystemStatusJson>),
    VersionChecked(CliResult<String>),

    // ---- action results ----
    ActionDone {
        id: String,
        kind: ActionKind,
        command: String,
        result: CliResult<()>,
    },

    // ---- log follower ----
    LogBacklog {
        id: String,
        lines: Vec<String>,
        error: Option<CliError>,
    },
    LogLine {
        id: String,
        line: String,
    },
    FollowExited {
        id: String,
    },

    // ---- inspect ----
    InspectLoaded {
        id: String,
        result: CliResult<String>,
    },

    // ---- pull ----
    PullLine {
        reference: String,
        line: String,
    },
    PullDone {
        reference: String,
        code: i32,
    },

    // ---- service start output ----
    ServiceStartLine(String),
    ServiceStartExited(i32),
}

/// Commands the UI emits from key handling. The UI never touches the Client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    SkipSplash,
    SwitchPane(Pane),
    NextPane,
    FocusDetail,
    Back, // esc: detail→list, clear filter, unzoom
    ToggleZoom,
    SetDetailTab(DetailTab),
    Move(isize),
    Top,
    Bottom,
    StartFilter,
    FilterChar(char),
    FilterBackspace,
    FilterCommit,
    OpenActionMenu,
    OpenHelp,
    OpenMessageLog,
    CloseOverlay,
    DismissBanner,
    Run(UiAction),
    ConfirmYes,
    /// Char typed while an overlay owns input (action menu key, pull input).
    OverlayChar(char),
    OverlayBackspace,
    OverlaySubmit,
    ScrollDetail(isize),
    /// Absolute detail scroll (used when a scroll-up interrupts follow).
    SetDetailScroll(u16),
    /// Absolute help-cheatsheet scroll; keymap clamps it to the drawn content.
    SetHelpScroll(u16),
    ScrollTop,
    ScrollBottom,
    ToggleFollow,
    /// Toggle log wrap vs truncated. Session-global, like follow.
    ToggleWrap,
    /// One-key service start on the takeover screen.
    StartService,
}

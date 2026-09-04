use crate::client::CliError;
use crate::client::model::{
    ContainerJson, ImageJson, NetworkJson, StatsJson, SystemStatusJson, VolumeJson,
};
use crate::engine::state::{ActionKind, DetailTab, Pane, UiAction};

type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum AppEvent {
    Containers(CliResult<Vec<ContainerJson>>),
    Images(CliResult<Vec<ImageJson>>),
    Volumes(CliResult<Vec<VolumeJson>>),
    Networks(CliResult<Vec<NetworkJson>>),
    Stats(CliResult<Vec<StatsJson>>),
    ServiceProbe(CliResult<SystemStatusJson>),
    VersionChecked(CliResult<String>),

    ActionDone {
        id: String,
        kind: ActionKind,
        command: String,
        result: CliResult<()>,
    },

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

    InspectLoaded {
        id: String,
        result: CliResult<String>,
    },

    PullLine {
        reference: String,
        line: String,
    },
    PullDone {
        reference: String,
        code: i32,
    },

    ServiceStartLine(String),
    ServiceStartExited(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    SkipSplash,
    SwitchPane(Pane),
    NextPane,
    FocusDetail,
    Back,
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
    OverlayChar(char),
    OverlayBackspace,
    OverlaySubmit,
    ScrollDetail(isize),
    SetDetailScroll(u16),
    SetHelpScroll(u16),
    ScrollTop,
    ScrollBottom,
    ToggleFollow,
    ToggleWrap,
    StartService,
}

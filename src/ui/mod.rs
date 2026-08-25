//! The Ratatui + tachyonfx shell: consumes `AppState`, emits `Command`s, and
//! owns the motion language (≤150ms, interruptible, `reduced-motion` kills all).

pub mod draw;
pub mod keymap;
pub mod log_view;
pub mod theme;

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use tachyonfx::{EffectManager, Interpolation, Motion, fx};

use crate::engine::state::{AppState, Overlay, Pane, Screen};
use crate::ui::draw::DrawInfo;
use crate::ui::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
enum FxKey {
    #[default]
    Ambient,
}

/// Discriminant snapshot used to trigger transition effects on state changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Snapshot {
    screen: Screen,
    pane: Pane,
    focus: crate::engine::state::Focus,
    overlay: Option<OverlayKind>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OverlayKind {
    ActionMenu,
    Modal, // confirm / help / message log / pull input all fade in the same way
}

fn overlay_kind(o: &Overlay) -> Option<OverlayKind> {
    match o {
        Overlay::None => None,
        Overlay::ActionMenu => Some(OverlayKind::ActionMenu),
        _ => Some(OverlayKind::Modal),
    }
}

pub struct Ui {
    pub theme: Theme,
    reduced_motion: bool,
    transitions: EffectManager<u8>,
    ambient: EffectManager<FxKey>,
    prev: Option<Snapshot>,
    last_toast_at: Option<std::time::Instant>,
    pub last_info: DrawInfo,
}

impl Ui {
    pub fn new(theme: Theme, reduced_motion: bool) -> Self {
        Self {
            theme,
            reduced_motion,
            transitions: EffectManager::default(),
            ambient: EffectManager::default(),
            prev: None,
            last_toast_at: None,
            last_info: DrawInfo::default(),
        }
    }

    /// Are non-ambient effects mid-flight (arms the 30fps frame ticker)?
    pub fn animating(&self, state: &AppState) -> bool {
        self.transitions.is_running()
            || state.screen == Screen::Splash
            || state.service_starting
            || state.pull.is_some()
    }

    /// Ambient hue drift wants a gentler repaint cadence.
    pub fn ambient_active(&self) -> bool {
        !self.reduced_motion
    }

    pub fn render(&mut self, frame: &mut Frame, state: &AppState, elapsed: Duration) {
        let info = draw::draw(frame, state, &self.theme);
        self.last_info = info;

        // transition effects on state changes — hard rules: ≤150ms, interruptible,
        // never delaying data (they postprocess an already-drawn buffer)
        let snap = Snapshot {
            screen: state.screen,
            pane: state.pane,
            focus: state.focus,
            overlay: overlay_kind(&state.overlay),
        };
        if !self.reduced_motion {
            if let Some(prev) = self.prev {
                if prev.screen == Screen::Splash && snap.screen == Screen::Main
                    || prev.screen == Screen::ServiceDown && snap.screen == Screen::Main
                {
                    self.transitions
                        .add_effect(fx::coalesce((150, Interpolation::QuadOut)));
                } else if prev.pane != snap.pane && snap.screen == Screen::Main {
                    self.transitions.add_effect(
                        fx::slide_in(
                            Motion::LeftToRight,
                            12,
                            0,
                            self.theme.bg(),
                            (140, Interpolation::QuadOut),
                        )
                        .with_area(info.body),
                    );
                } else if prev.overlay != snap.overlay && snap.overlay.is_some() {
                    let effect = if snap.overlay == Some(OverlayKind::ActionMenu) {
                        // bottom-sheet slide for the action menu
                        fx::slide_in(
                            Motion::DownToUp,
                            6,
                            0,
                            self.theme.bg(),
                            (120, Interpolation::QuadOut),
                        )
                    } else {
                        fx::fade_from(
                            self.theme.dim(),
                            self.theme.bg(),
                            (100, Interpolation::QuadOut),
                        )
                    };
                    self.transitions.add_effect(effect);
                } else if prev.focus != snap.focus && snap.screen == Screen::Main {
                    // focus transition: brief fade on the newly focused split
                    self.transitions.add_effect(
                        fx::fade_from(
                            self.theme.dim(),
                            self.theme.bg(),
                            (100, Interpolation::QuadOut),
                        )
                        .with_area(info.body),
                    );
                }
                // toast slide-in on the bottom bar
                let toast_at = state.toast.as_ref().map(|t| t.at);
                if toast_at.is_some() && toast_at != self.last_toast_at {
                    self.transitions.add_effect(
                        fx::slide_in(
                            Motion::LeftToRight,
                            8,
                            0,
                            self.theme.bar(),
                            (120, Interpolation::QuadOut),
                        )
                        .with_area(info.bottom),
                    );
                }
                self.last_toast_at = toast_at;
            }
        }
        self.prev = Some(snap);

        let area = frame.area();
        self.transitions
            .process_effects(elapsed.into(), frame.buffer_mut(), area);

        // ambient wordmark hue drift. The 2800ms period is exempt from the ADR's
        // ≤150ms rule: that rule governs transition micro-motion; ambient effects
        // are separately prototype-gated (ADR 0001), passed the gate, and are
        // killed by `reduced-motion` like everything else.
        if !self.reduced_motion && state.screen == Screen::Main && !self.ambient.is_running() {
            let effect = fx::repeating(fx::ping_pong(fx::hsl_shift_fg(
                [50.0, 10.0, 6.0],
                (2800, Interpolation::SineInOut),
            )))
            .with_area(Rect {
                height: 1,
                width: 9,
                ..info.header
            });
            let effect = self.ambient.unique(FxKey::Ambient, effect);
            self.ambient.add_effect(effect);
        }
        if state.screen == Screen::Main {
            self.ambient
                .process_effects(elapsed.into(), frame.buffer_mut(), area);
        }
    }

    /// Full-redraw effect after returning from exec.
    pub fn after_exec(&mut self) {
        if !self.reduced_motion {
            self.transitions
                .add_effect(fx::coalesce((150, Interpolation::QuadOut)));
        }
    }
}

//! Key → Command translation. Pure: reads state, never mutates it, so the whole
//! input scheme is testable without a terminal.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::engine::state::AppState;
use crate::engine::{Command, DetailTab, Focus, Overlay, Pane, Screen};
use crate::ui::draw::DrawInfo;

/// Some keys need what the last frame drew: a scroll-up during follow pins to
/// the log line then at the top of the viewport, and a help scroll stops at the
/// end of the cheatsheet as it was laid out. `drawn` carries both.
pub fn map_key(state: &AppState, key: KeyEvent, drawn: &DrawInfo) -> Vec<Command> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return vec![Command::Quit];
    }

    // splash: any key skips
    if state.screen == Screen::Splash {
        return vec![Command::SkipSplash];
    }

    // service-down takeover
    if state.screen == Screen::ServiceDown {
        return match key.code {
            KeyCode::Char('s') => vec![Command::StartService],
            KeyCode::Char('q') => vec![Command::Quit],
            KeyCode::Char('m') => vec![Command::OpenMessageLog],
            _ => vec![],
        };
    }

    // overlays capture input first
    match &state.overlay {
        Overlay::Confirm { .. } => {
            return match key.code {
                KeyCode::Char('y') => vec![Command::ConfirmYes],
                KeyCode::Esc | KeyCode::Char('n') => vec![Command::CloseOverlay],
                _ => vec![],
            };
        }
        Overlay::Help => {
            // one clamped, scrollable cheatsheet — no shorter floor variant
            let to = |delta: isize| -> Vec<Command> {
                let v = if delta < 0 {
                    state.help_scroll.saturating_sub((-delta) as u16)
                } else {
                    state.help_scroll.saturating_add(delta as u16)
                };
                vec![Command::SetHelpScroll(v.min(drawn.help_max_scroll))]
            };
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Char('m') => {
                    vec![Command::CloseOverlay]
                }
                KeyCode::Char('j') | KeyCode::Down => to(1),
                KeyCode::Char('k') | KeyCode::Up => to(-1),
                KeyCode::PageDown => to(10),
                KeyCode::PageUp => to(-10),
                KeyCode::Char('g') => vec![Command::SetHelpScroll(0)],
                KeyCode::Char('G') => vec![Command::SetHelpScroll(drawn.help_max_scroll)],
                _ => vec![],
            };
        }
        Overlay::MessageLog => {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Char('m') => {
                    vec![Command::CloseOverlay]
                }
                _ => vec![],
            };
        }
        Overlay::ActionMenu => {
            return match key.code {
                KeyCode::Esc => vec![Command::CloseOverlay],
                KeyCode::Char(c) => vec![Command::OverlayChar(c)],
                _ => vec![],
            };
        }
        Overlay::PullInput { .. } => {
            return match key.code {
                KeyCode::Esc => vec![Command::CloseOverlay],
                KeyCode::Enter => vec![Command::OverlaySubmit],
                KeyCode::Backspace => vec![Command::OverlayBackspace],
                KeyCode::Char(c) => vec![Command::OverlayChar(c)],
                _ => vec![],
            };
        }
        Overlay::None => {}
    }

    // filter input mode
    if state.filter_input {
        return match key.code {
            KeyCode::Esc => vec![Command::Back],
            KeyCode::Enter => vec![Command::FilterCommit],
            KeyCode::Backspace => vec![Command::FilterBackspace],
            KeyCode::Char(c) => vec![Command::FilterChar(c)],
            _ => vec![],
        };
    }

    let logs_tab = state.pane == Pane::Containers && state.detail_tab == DetailTab::Logs;

    // scroll-up while following: pin to the current position, then move
    let unfollow_scroll = |delta: u16| -> Vec<Command> {
        if logs_tab && state.follow {
            vec![Command::SetDetailScroll(
                drawn.log_scroll.saturating_sub(delta),
            )]
        } else {
            vec![Command::ScrollDetail(-(delta as isize))]
        }
    };

    // global keys
    match key.code {
        KeyCode::Char('q') => vec![Command::Quit],
        KeyCode::Char('1') => vec![Command::SwitchPane(Pane::Containers)],
        KeyCode::Char('2') => vec![Command::SwitchPane(Pane::Images)],
        KeyCode::Char('3') => vec![Command::SwitchPane(Pane::Volumes)],
        KeyCode::Tab => vec![Command::NextPane],
        KeyCode::Char('?') => vec![Command::OpenHelp],
        KeyCode::Char('m') => vec![Command::OpenMessageLog],
        KeyCode::Char('b') => vec![Command::DismissBanner],
        KeyCode::Char('f') => vec![Command::ToggleZoom],
        KeyCode::Char('F') if logs_tab => vec![Command::ToggleFollow],
        KeyCode::Char('w') => vec![Command::ToggleWrap],
        KeyCode::Char('/') if state.focus == Focus::List => vec![Command::StartFilter],
        KeyCode::Enter if state.focus == Focus::List => vec![Command::FocusDetail],
        KeyCode::Esc => vec![Command::Back],
        KeyCode::Char(' ') if state.focus == Focus::List => vec![Command::OpenActionMenu],
        // PgUp/PgDn scroll the detail pane without switching focus
        KeyCode::PageDown => vec![Command::ScrollDetail(10)],
        KeyCode::PageUp => unfollow_scroll(10),
        KeyCode::Char('l') if state.pane == Pane::Containers => {
            vec![Command::SetDetailTab(DetailTab::Logs)]
        }
        KeyCode::Char('i') if state.pane == Pane::Containers => {
            vec![Command::SetDetailTab(DetailTab::Inspect)]
        }
        _ => match state.focus {
            Focus::List => match key.code {
                KeyCode::Char('j') | KeyCode::Down => vec![Command::Move(1)],
                KeyCode::Char('k') | KeyCode::Up => vec![Command::Move(-1)],
                KeyCode::Char('g') => vec![Command::Top],
                KeyCode::Char('G') => vec![Command::Bottom],
                KeyCode::Char(c) => state
                    .available_actions()
                    .iter()
                    .find(|a| a.key == c)
                    .map(|a| vec![Command::Run(a.action)])
                    .unwrap_or_default(),
                _ => vec![],
            },
            Focus::Detail => match key.code {
                KeyCode::Char('j') | KeyCode::Down => vec![Command::ScrollDetail(1)],
                KeyCode::Char('k') | KeyCode::Up => unfollow_scroll(1),
                KeyCode::Char('g') => unfollow_scroll(u16::MAX),
                KeyCode::Char('G') => {
                    if logs_tab && !state.follow {
                        vec![Command::ToggleFollow] // re-follow snaps to the tail
                    } else {
                        vec![Command::ScrollBottom]
                    }
                }
                _ => vec![],
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::UiAction;
    use crossterm::event::KeyEvent;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// What the last frame drew: the top log line, and the help scroll ceiling.
    fn drawn(log_scroll: u16, help_max_scroll: u16) -> DrawInfo {
        DrawInfo {
            log_scroll,
            help_max_scroll,
            ..DrawInfo::default()
        }
    }

    fn main_state() -> AppState {
        let mut s = AppState::new(true);
        s.containers.push(crate::engine::state::ContainerEntry {
            id: "web".into(),
            image: "alpine:latest".into(),
            state: "running".into(),
            created: None,
            cpus: None,
            volumes: vec![],
            cpu_percent: None,
            mem_bytes: None,
            telemetry: std::collections::VecDeque::new(),
            pending: None,
        });
        s.clamp_selection();
        s
    }

    #[test]
    fn any_key_skips_the_splash() {
        let mut s = main_state();
        s.screen = Screen::Splash;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('x')), &drawn(0, 0)),
            vec![Command::SkipSplash]
        );
    }

    #[test]
    fn service_down_screen_only_accepts_start_quit_and_message_log() {
        let mut s = main_state();
        s.screen = Screen::ServiceDown;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('s')), &drawn(0, 0)),
            vec![Command::StartService]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('q')), &drawn(0, 0)),
            vec![Command::Quit]
        );
        assert_eq!(map_key(&s, key(KeyCode::Char('d')), &drawn(0, 0)), vec![]);
    }

    #[test]
    fn confirm_overlay_captures_y_and_esc() {
        let mut s = main_state();
        s.overlay = Overlay::Confirm {
            command: "container kill web".into(),
            action: crate::engine::ActionKind::Kill,
            target: "web".into(),
        };
        assert_eq!(
            map_key(&s, key(KeyCode::Char('y')), &drawn(0, 0)),
            vec![Command::ConfirmYes]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Esc), &drawn(0, 0)),
            vec![Command::CloseOverlay]
        );
        // other keys do nothing — no accidental direct actions under a modal
        assert_eq!(map_key(&s, key(KeyCode::Char('d')), &drawn(0, 0)), vec![]);
    }

    #[test]
    fn direct_action_keys_resolve_via_available_actions() {
        let s = main_state();
        // running container: 's' is stop, 'K' is kill
        assert_eq!(
            map_key(&s, key(KeyCode::Char('s')), &drawn(0, 0)),
            vec![Command::Run(UiAction::Stop)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('K')), &drawn(0, 0)),
            vec![Command::Run(UiAction::Kill)]
        );
        // 'x' is bound to nothing
        assert_eq!(map_key(&s, key(KeyCode::Char('x')), &drawn(0, 0)), vec![]);
    }

    #[test]
    fn scrolling_up_during_follow_pins_to_the_drawn_position() {
        let mut s = main_state();
        s.focus = Focus::Detail;
        s.follow = true;
        let cmds = map_key(&s, key(KeyCode::Char('k')), &drawn(42, 0));
        assert_eq!(cmds, vec![Command::SetDetailScroll(41)]);
    }

    #[test]
    fn big_g_reenables_follow_when_paused() {
        let mut s = main_state();
        s.focus = Focus::Detail;
        s.follow = false;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('G')), &drawn(0, 0)),
            vec![Command::ToggleFollow]
        );
    }

    #[test]
    fn filter_input_swallows_action_keys() {
        let mut s = main_state();
        s.filter_input = true;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('d')), &drawn(0, 0)),
            vec![Command::FilterChar('d')]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Esc), &drawn(0, 0)),
            vec![Command::Back]
        );
    }

    #[test]
    fn pgdn_scrolls_detail_without_switching_focus() {
        let mut s = main_state();
        s.detail_tab = DetailTab::Inspect;
        assert_eq!(
            map_key(&s, key(KeyCode::PageDown), &drawn(0, 0)),
            vec![Command::ScrollDetail(10)]
        );
        assert_eq!(s.focus, Focus::List);
    }

    #[test]
    fn w_toggles_wrap_from_list_or_detail() {
        let mut s = main_state();
        assert_eq!(
            map_key(&s, key(KeyCode::Char('w')), &drawn(0, 0)),
            vec![Command::ToggleWrap]
        );
        s.focus = Focus::Detail;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('w')), &drawn(0, 0)),
            vec![Command::ToggleWrap]
        );
    }

    #[test]
    fn w_in_filter_is_a_filter_char() {
        let mut s = main_state();
        s.filter_input = true;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('w')), &drawn(0, 0)),
            vec![Command::FilterChar('w')]
        );
    }

    #[test]
    fn help_scrolls_and_stops_at_the_end_of_the_cheatsheet() {
        let mut s = main_state();
        s.overlay = Overlay::Help;
        // j/k move one row, clamped to what the last draw could scroll
        assert_eq!(
            map_key(&s, key(KeyCode::Char('j')), &drawn(0, 4)),
            vec![Command::SetHelpScroll(1)]
        );
        s.help_scroll = 4;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('j')), &drawn(0, 4)),
            vec![Command::SetHelpScroll(4)],
            "cannot scroll past the last row"
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('k')), &drawn(0, 4)),
            vec![Command::SetHelpScroll(3)]
        );
        s.help_scroll = 0;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('k')), &drawn(0, 4)),
            vec![Command::SetHelpScroll(0)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::PageDown), &drawn(0, 4)),
            vec![Command::SetHelpScroll(4)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('G')), &drawn(0, 4)),
            vec![Command::SetHelpScroll(4)]
        );
        // and it still closes on every key that closed it before
        for c in [
            KeyCode::Esc,
            KeyCode::Char('q'),
            KeyCode::Char('?'),
            KeyCode::Char('m'),
        ] {
            assert_eq!(
                map_key(&s, key(c), &drawn(0, 4)),
                vec![Command::CloseOverlay]
            );
        }
    }

    #[test]
    fn a_help_that_fits_does_not_scroll() {
        let mut s = main_state();
        s.overlay = Overlay::Help;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('j')), &drawn(0, 0)),
            vec![Command::SetHelpScroll(0)]
        );
    }

    #[test]
    fn the_message_log_is_unchanged_at_every_size() {
        let mut s = main_state();
        s.overlay = Overlay::MessageLog;
        for c in ['q', 'm', '?'] {
            assert_eq!(
                map_key(&s, key(KeyCode::Char(c)), &drawn(0, 0)),
                vec![Command::CloseOverlay]
            );
        }
        assert_eq!(map_key(&s, key(KeyCode::Char('j')), &drawn(0, 0)), vec![]);
    }

    #[test]
    fn l_and_i_still_switch_tabs_while_the_sheet_is_open() {
        let mut s = main_state();
        s.overlay = Overlay::ActionMenu;
        // the sheet routes them as overlay chars; the engine runs the tab jump
        assert_eq!(
            map_key(&s, key(KeyCode::Char('l')), &drawn(0, 0)),
            vec![Command::OverlayChar('l')]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('i')), &drawn(0, 0)),
            vec![Command::OverlayChar('i')]
        );
        // and directly while it is closed
        s.overlay = Overlay::None;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('l')), &drawn(0, 0)),
            vec![Command::SetDetailTab(DetailTab::Logs)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('i')), &drawn(0, 0)),
            vec![Command::SetDetailTab(DetailTab::Inspect)]
        );
    }
}

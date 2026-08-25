//! Key → Command translation. Pure: reads state, never mutates it, so the whole
//! input scheme is testable without a terminal.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::engine::state::AppState;
use crate::engine::{Command, DetailTab, Focus, Overlay, Pane, Screen};

/// Raw log line at the top of the logs viewport at last draw (needed to convert
/// a scroll-up during follow into an absolute position).
pub fn map_key(state: &AppState, key: KeyEvent, last_log_scroll: u16) -> Vec<Command> {
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
        Overlay::Help | Overlay::MessageLog => {
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
                last_log_scroll.saturating_sub(delta),
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
            map_key(&s, key(KeyCode::Char('x')), 0),
            vec![Command::SkipSplash]
        );
    }

    #[test]
    fn service_down_screen_only_accepts_start_quit_and_message_log() {
        let mut s = main_state();
        s.screen = Screen::ServiceDown;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('s')), 0),
            vec![Command::StartService]
        );
        assert_eq!(map_key(&s, key(KeyCode::Char('q')), 0), vec![Command::Quit]);
        assert_eq!(map_key(&s, key(KeyCode::Char('d')), 0), vec![]);
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
            map_key(&s, key(KeyCode::Char('y')), 0),
            vec![Command::ConfirmYes]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Esc), 0),
            vec![Command::CloseOverlay]
        );
        // other keys do nothing — no accidental direct actions under a modal
        assert_eq!(map_key(&s, key(KeyCode::Char('d')), 0), vec![]);
    }

    #[test]
    fn direct_action_keys_resolve_via_available_actions() {
        let s = main_state();
        // running container: 's' is stop, 'K' is kill
        assert_eq!(
            map_key(&s, key(KeyCode::Char('s')), 0),
            vec![Command::Run(UiAction::Stop)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('K')), 0),
            vec![Command::Run(UiAction::Kill)]
        );
        // 'x' is bound to nothing
        assert_eq!(map_key(&s, key(KeyCode::Char('x')), 0), vec![]);
    }

    #[test]
    fn scrolling_up_during_follow_pins_to_the_drawn_position() {
        let mut s = main_state();
        s.focus = Focus::Detail;
        s.follow = true;
        let cmds = map_key(&s, key(KeyCode::Char('k')), 42);
        assert_eq!(cmds, vec![Command::SetDetailScroll(41)]);
    }

    #[test]
    fn big_g_reenables_follow_when_paused() {
        let mut s = main_state();
        s.focus = Focus::Detail;
        s.follow = false;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('G')), 0),
            vec![Command::ToggleFollow]
        );
    }

    #[test]
    fn filter_input_swallows_action_keys() {
        let mut s = main_state();
        s.filter_input = true;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('d')), 0),
            vec![Command::FilterChar('d')]
        );
        assert_eq!(map_key(&s, key(KeyCode::Esc), 0), vec![Command::Back]);
    }

    #[test]
    fn pgdn_scrolls_detail_without_switching_focus() {
        let mut s = main_state();
        s.detail_tab = DetailTab::Inspect;
        assert_eq!(
            map_key(&s, key(KeyCode::PageDown), 0),
            vec![Command::ScrollDetail(10)]
        );
        assert_eq!(s.focus, Focus::List);
    }

    #[test]
    fn w_toggles_wrap_from_list_or_detail() {
        let mut s = main_state();
        assert_eq!(
            map_key(&s, key(KeyCode::Char('w')), 0),
            vec![Command::ToggleWrap]
        );
        s.focus = Focus::Detail;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('w')), 0),
            vec![Command::ToggleWrap]
        );
    }

    #[test]
    fn w_in_filter_is_a_filter_char() {
        let mut s = main_state();
        s.filter_input = true;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('w')), 0),
            vec![Command::FilterChar('w')]
        );
    }
}

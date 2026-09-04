use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::engine::state::AppState;
use crate::engine::{Command, DetailTab, Focus, Overlay, Pane, Screen};
use crate::ui::draw::DrawInfo;

pub fn map_key(state: &AppState, key: KeyEvent, drawn: &DrawInfo) -> Vec<Command> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return vec![Command::Quit];
    }

    if state.screen == Screen::Splash {
        return vec![Command::SkipSplash];
    }

    if state.screen == Screen::ServiceDown {
        return match key.code {
            KeyCode::Char('s') => vec![Command::StartService],
            KeyCode::Char('q') => vec![Command::Quit],
            KeyCode::Char('m') => vec![Command::OpenMessageLog],
            _ => vec![],
        };
    }

    match &state.overlay {
        Overlay::Confirm { .. } => {
            return match key.code {
                KeyCode::Char('y') => vec![Command::ConfirmYes],
                KeyCode::Esc | KeyCode::Char('n') => vec![Command::CloseOverlay],
                _ => vec![],
            };
        }
        Overlay::Help => {
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
        Overlay::PullInput { .. }
        | Overlay::TagInput { .. }
        | Overlay::CreateVolumeInput { .. } => {
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

    let unfollow_scroll = |delta: u16| -> Vec<Command> {
        if logs_tab && state.follow {
            vec![Command::SetDetailScroll(
                drawn.log_scroll.saturating_sub(delta),
            )]
        } else {
            vec![Command::ScrollDetail(-(delta as isize))]
        }
    };

    match key.code {
        KeyCode::Char('q') => vec![Command::Quit],
        KeyCode::Char('1') => vec![Command::SwitchPane(Pane::Containers)],
        KeyCode::Char('2') => vec![Command::SwitchPane(Pane::Images)],
        KeyCode::Char('3') => vec![Command::SwitchPane(Pane::Volumes)],
        KeyCode::Char('4') => vec![Command::SwitchPane(Pane::Networks)],
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
                        vec![Command::ToggleFollow]
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
            networks: vec![],
            cpu_percent: None,
            mem_bytes: None,
            telemetry: std::collections::VecDeque::new(),
            pending: None,
        });
        s.clamp_selection();
        s
    }

    #[test]
    fn digit_keys_switch_all_four_panes() {
        let s = main_state();
        assert_eq!(
            map_key(&s, key(KeyCode::Char('1')), &drawn(0, 0)),
            vec![Command::SwitchPane(Pane::Containers)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('2')), &drawn(0, 0)),
            vec![Command::SwitchPane(Pane::Images)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('3')), &drawn(0, 0)),
            vec![Command::SwitchPane(Pane::Volumes)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('4')), &drawn(0, 0)),
            vec![Command::SwitchPane(Pane::Networks)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Tab), &drawn(0, 0)),
            vec![Command::NextPane]
        );
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
        assert_eq!(map_key(&s, key(KeyCode::Char('d')), &drawn(0, 0)), vec![]);
    }

    #[test]
    fn direct_action_keys_resolve_via_available_actions() {
        let s = main_state();
        assert_eq!(
            map_key(&s, key(KeyCode::Char('s')), &drawn(0, 0)),
            vec![Command::Run(UiAction::Stop)]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('K')), &drawn(0, 0)),
            vec![Command::Run(UiAction::Kill)]
        );
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

    fn documented_keys() -> Vec<String> {
        crate::ui::help::HELP
            .iter()
            .flat_map(|row| row.keys.split([' ', ',']))
            .flat_map(|tok| {
                if tok.len() > 1 {
                    tok.split('/').collect::<Vec<_>>()
                } else {
                    vec![tok]
                }
            })
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn help_token(code: KeyCode) -> Option<String> {
        Some(match code {
            KeyCode::Char(' ') => "space".into(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Tab => "tab".into(),
            KeyCode::Enter => "enter".into(),
            KeyCode::Esc => "esc".into(),
            KeyCode::PageUp => "pgup".into(),
            KeyCode::PageDown => "pgdn".into(),
            _ => return None,
        })
    }

    fn cheatsheet_states() -> Vec<AppState> {
        let list = main_state();

        let mut detail = main_state();
        detail.focus = Focus::Detail;
        detail.detail_tab = DetailTab::Logs;

        let mut images = main_state();
        images.pane = Pane::Images;
        images.images.push(crate::engine::state::ImageEntry {
            reference: "alpine:latest".into(),
            size: None,
            created: None,
            pending: None,
        });
        images.clamp_selection();

        let mut volumes = main_state();
        volumes.pane = Pane::Volumes;
        volumes.clamp_selection();

        let mut networks = main_state();
        networks.pane = Pane::Networks;
        networks.networks.push(crate::engine::state::NetworkEntry {
            name: "default".into(),
            mode: "nat".into(),
            ipv4_subnet: Some("192.168.64.0/24".into()),
            builtin: true,
            attached: vec![],
            created: None,
        });
        networks.clamp_selection();

        vec![list, detail, images, volumes, networks]
    }

    fn is_bound(code: KeyCode) -> bool {
        cheatsheet_states()
            .iter()
            .any(|s| !map_key(s, key(code), &drawn(0, 0)).is_empty())
    }

    #[test]
    fn every_key_the_main_keymap_handles_is_on_the_cheatsheet() {
        const UNDOCUMENTED: &[KeyCode] = &[KeyCode::Char('?'), KeyCode::Up, KeyCode::Down];

        let documented = documented_keys();
        let candidates = (' '..='~').map(KeyCode::Char).chain([
            KeyCode::Tab,
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::Up,
            KeyCode::Down,
        ]);

        for code in candidates {
            if UNDOCUMENTED.contains(&code) || !is_bound(code) {
                continue;
            }
            let token = help_token(code)
                .unwrap_or_else(|| panic!("{code:?} is handled but has no cheatsheet spelling"));
            assert!(
                documented.contains(&token),
                "`{token}` ({code:?}) does something but is not on the cheatsheet"
            );
        }
    }

    #[test]
    fn c_creates_a_volume_from_the_volumes_pane() {
        let mut s = main_state();
        s.pane = Pane::Volumes;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('c')), &drawn(0, 0)),
            vec![Command::Run(UiAction::Create)]
        );
        s.pane = Pane::Containers;
        assert_eq!(map_key(&s, key(KeyCode::Char('c')), &drawn(0, 0)), vec![]);
    }

    #[test]
    fn the_question_mark_opens_the_cheatsheet() {
        let s = main_state();
        assert_eq!(
            map_key(&s, key(KeyCode::Char('?')), &drawn(0, 0)),
            vec![Command::OpenHelp]
        );
    }

    #[test]
    fn the_cheatsheet_lists_nothing_that_does_nothing() {
        for token in documented_keys() {
            let code = match token.as_str() {
                "space" => KeyCode::Char(' '),
                "tab" => KeyCode::Tab,
                "enter" => KeyCode::Enter,
                "esc" => KeyCode::Esc,
                "pgup" => KeyCode::PageUp,
                "pgdn" => KeyCode::PageDown,
                t if t.chars().count() == 1 => KeyCode::Char(t.chars().next().unwrap()),
                _ => continue,
            };
            assert!(
                is_bound(code),
                "the cheatsheet documents `{token}`, but it is bound to nothing"
            );
        }
    }

    #[test]
    fn t_on_the_images_pane_runs_tag() {
        let mut s = main_state();
        s.pane = Pane::Images;
        s.images.push(crate::engine::state::ImageEntry {
            reference: "alpine:latest".into(),
            size: None,
            created: None,
            pending: None,
        });
        s.clamp_selection();
        assert_eq!(
            map_key(&s, key(KeyCode::Char('t')), &drawn(0, 0)),
            vec![Command::Run(UiAction::Tag)]
        );
        s.pane = Pane::Containers;
        assert_eq!(map_key(&s, key(KeyCode::Char('t')), &drawn(0, 0)), vec![]);
    }

    #[test]
    fn l_and_i_still_switch_tabs_while_the_sheet_is_open() {
        let mut s = main_state();
        s.overlay = Overlay::ActionMenu;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('l')), &drawn(0, 0)),
            vec![Command::OverlayChar('l')]
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('i')), &drawn(0, 0)),
            vec![Command::OverlayChar('i')]
        );
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

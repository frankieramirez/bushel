use std::sync::Arc;

use bushel::client::{Client, model::ContainerJson};
use bushel::engine::{
    AppEvent, AppState, Command, DetailTab, Engine, Focus, Overlay, Pane, Screen, UiAction,
};
use bushel::runner::{MockRunner, Output};
use bushel::ui::{Ui, draw, keymap, theme::Theme};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use tokio::sync::mpsc;

fn render(state: &AppState, width: u16, height: u16) -> (String, draw::DrawInfo) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut info = draw::DrawInfo::default();
    terminal
        .draw(|frame| {
            info = draw::draw(
                frame,
                state,
                &Theme {
                    truecolor: false,
                    ascii: true,
                    reduced_motion: false,
                },
            );
        })
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    (text, info)
}

fn assert_health_banner(pane: Pane, gist: &str) {
    for layout in [
        bushel::config::LayoutMode::Rail,
        bushel::config::LayoutMode::Table,
    ] {
        let mut state = AppState::new(true);
        state.screen = Screen::Main;
        state.config.layout = layout;
        state.pane = pane;
        if pane == Pane::Containers {
            let containers: Vec<ContainerJson> =
                serde_json::from_str(include_str!("../fixtures/1.2.0/ls.json")).unwrap();
            state.update_containers(&containers);
            for _ in 0..bushel::engine::state::DEGRADED_THRESHOLD {
                state.stats_health.fail(gist.into());
            }
        } else {
            state.reads[pane.index()] =
                bushel::engine::state::ReadStatus::Failed { gist: gist.into() };
        }
        for (width, height) in [(55, 20), (120, 40)] {
            let text = render(&state, width, height).0;
            let banner = if pane == Pane::Containers {
                format!("stats unavailable: {gist}")
            } else {
                format!("{} list failed: {gist}", pane.title())
            };
            assert!(text.contains(&banner), "{pane:?} {width}x{height}: {text}");
            if gist.is_ascii() {
                assert!(text.is_ascii(), "{pane:?} {width}x{height}: {text}");
            }
            if width == 120 {
                assert!(text.contains(". [m] log"), "{text}");
                if pane != Pane::Containers {
                    assert!(text.contains("- showing last good state"), "{text}");
                }
            }
        }
    }
}

#[test]
fn stats_failure_banner_uses_ascii_chrome() {
    assert_health_banner(Pane::Containers, "CLI timeout");
}

#[test]
fn images_failure_banner_uses_ascii_chrome() {
    assert_health_banner(Pane::Images, "CLI timeout");
}

#[test]
fn volumes_failure_banner_uses_ascii_chrome() {
    assert_health_banner(Pane::Volumes, "CLI timeout");
}

#[test]
fn networks_failure_banner_uses_ascii_chrome() {
    assert_health_banner(Pane::Networks, "CLI timeout");
}

#[test]
fn health_banners_preserve_external_unicode_errors() {
    for pane in Pane::all() {
        assert_health_banner(pane, "external → é — ·");
    }
}

#[test]
fn bounded_log_ring_keeps_the_paused_line_when_old_entries_are_evicted() {
    let mut state = AppState::new(true);
    state.follow = false;
    for i in 0..bushel::engine::state::LOG_RING_CAP {
        state.push_log_line(format!("line {i}"));
    }
    state.detail_scroll = 10;
    state.push_log_line("new line".into());
    assert_eq!(state.log_lines.len(), bushel::engine::state::LOG_RING_CAP);
    assert_eq!(state.log_lines.front().unwrap(), "line 1");
    assert_eq!(state.log_lines[state.detail_scroll as usize], "line 10");
    assert_eq!(state.log_lines.back().unwrap(), "new line");
}

#[tokio::test]
async fn filtering_keeps_a_visible_selection_and_unicode_backspace_removes_a_grapheme() {
    let (tx, _rx) = mpsc::channel(64);
    let mut engine = Engine::new(Client::new(Arc::new(MockRunner::new())), tx, true);
    engine.state.pane = Pane::Containers;
    let containers: Vec<ContainerJson> =
        serde_json::from_str(include_str!("../fixtures/1.2.0/ls.json")).unwrap();
    engine.state.update_containers(&containers);
    engine.dispatch(Command::StartFilter);
    for c in "old-batch".chars() {
        engine.dispatch(Command::FilterChar(c));
    }
    assert_eq!(engine.state.selected_container().unwrap().id, "old-batch");
    engine.dispatch(Command::FilterChar('👩'));
    engine.dispatch(Command::FilterChar('\u{200d}'));
    engine.dispatch(Command::FilterChar('💻'));
    engine.dispatch(Command::FilterBackspace);
    assert_eq!(engine.state.filter, "old-batch");
    assert_eq!(engine.state.selected_container().unwrap().id, "old-batch");
    engine.shutdown();
}

#[tokio::test]
async fn actual_action_and_poll_messages_render_ascii_without_rewriting_external_text() {
    let mock = Arc::new(MockRunner::new());
    mock.on(&["start", "old-batch"], Output::ok(""));
    mock.on(&["stop", "old-batch"], Output::fail(1, "external failure"));
    for id in ["qtest", "old-batch"] {
        mock.on(&["inspect", id], Output::ok("{}"));
    }
    let (tx, mut rx) = mpsc::channel(64);
    let mut engine = Engine::new(Client::new(Arc::clone(&mock)), tx, true);
    engine.state.config.ascii = true;
    engine.state.detail_tab = DetailTab::Inspect;
    let mut containers: Vec<ContainerJson> =
        serde_json::from_str(include_str!("../fixtures/1.2.0/ls.json")).unwrap();
    engine.apply(AppEvent::Containers(1, Ok(containers.clone())));
    engine.state.messages.clear();
    engine.dispatch(Command::Bottom);
    engine.dispatch(Command::Run(UiAction::Start));
    apply_action_completion(&mut engine, &mut rx).await;
    assert!(
        engine
            .state
            .messages
            .iter()
            .any(|m| m.contains("awaiting poll confirmation"))
    );
    for container in &mut containers {
        container.status.state = "running".into();
    }
    engine.apply(AppEvent::Containers(100, Ok(containers.clone())));
    assert!(
        engine
            .state
            .messages
            .iter()
            .any(|m| m.contains("stopped -> running"))
    );
    engine.dispatch(Command::OpenMessageLog);
    for (width, height) in [(55, 20), (80, 24), (120, 40)] {
        let (text, _) = render(&engine.state, width, height);
        assert!(text.is_ascii(), "{width}x{height}: {text}");
        assert!(text.contains("stopped -> running"), "{text}");
        assert!(text.contains("$ container start old-batch -> ok"), "{text}");
    }

    // Raw CLI stderr and user identifiers retain their original Unicode.
    engine.dispatch(Command::CloseOverlay);
    engine.dispatch(Command::Top);
    engine.dispatch(Command::Run(UiAction::Stop));
    apply_action_completion(&mut engine, &mut rx).await;
    engine.dispatch(Command::OpenMessageLog);
    for (width, height) in [(55, 20), (80, 24), (120, 40)] {
        let (text, _) = render(&engine.state, width, height);
        assert!(text.is_ascii(), "{width}x{height}: {text}");
        assert!(text.contains("-> failed"), "{text}");
    }
    mock.set(&["stop", "old-batch"], Output::fail(1, "external → é"));
    engine.dispatch(Command::CloseOverlay);
    engine.dispatch(Command::Run(UiAction::Stop));
    apply_action_completion(&mut engine, &mut rx).await;
    engine.dispatch(Command::OpenMessageLog);
    assert!(render(&engine.state, 120, 40).0.contains("external → é"));
    engine.dispatch(Command::CloseOverlay);
    containers[0].id = "job→é".into();
    engine.apply(AppEvent::Containers(101, Ok(containers)));
    engine.dispatch(Command::OpenMessageLog);
    assert!(render(&engine.state, 120, 40).0.contains("job→é: appeared"));
    engine.shutdown();
}

async fn apply_action_completion(
    engine: &mut Engine<MockRunner>,
    rx: &mut mpsc::Receiver<AppEvent>,
) {
    loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("action completed")
            .expect("Engine sender remains alive");
        let done = matches!(event, AppEvent::ActionDone { .. });
        engine.apply(event);
        if done {
            break;
        }
    }
}

#[test]
fn message_navigation_reaches_old_wrapped_stderr_and_clamps_after_resize() {
    let mut state = AppState::new(true);
    state.overlay = Overlay::MessageLog;
    state.messages = (0..100)
        .map(|i| format!("message {i}\n{}", "x".repeat(100)))
        .collect();
    for (width, height) in [(55, 20), (120, 40)] {
        let (top, info) = render(&state, width, height);
        assert!(top.contains("message 99"));
        assert!(info.message_max_scroll > 0);
        assert_eq!(
            keymap::map_key(
                &state,
                KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE),
                &info
            ),
            [Command::SetMessageScroll(info.message_max_scroll)]
        );
        state.message_scroll = usize::MAX;
        let (bottom, _) = render(&state, width, height);
        assert!(bottom.contains("message 0"));
        state.message_scroll = 0;
    }
}

#[tokio::test]
async fn quit_wait_survives_service_loss_and_retains_its_command_choices() {
    let (tx, _rx) = mpsc::channel(64);
    let mock = Arc::new(MockRunner::new());
    mock.on(&["stop", "qtest"], Output::ok(""));
    let mut engine = Engine::new(Client::new(mock), tx, true);
    let containers: Vec<ContainerJson> =
        serde_json::from_str(include_str!("../fixtures/1.2.0/ls.json")).unwrap();
    engine.apply(AppEvent::Containers(1, Ok(containers)));
    engine.dispatch(Command::Top);
    engine.dispatch(Command::Run(UiAction::Stop));
    engine.dispatch(Command::Quit);
    engine.dispatch(Command::WaitAndQuit);
    engine.apply(AppEvent::ServiceProbe(Err(
        bushel::client::CliError::ServiceDown {
            raw: "service down".into(),
        },
    )));
    assert_eq!(engine.state.screen, Screen::ServiceDown);
    assert!(engine.state.quitting);
    assert!(matches!(engine.state.overlay, Overlay::QuitConfirm { .. }));
    for (width, height) in [(55, 20), (120, 40)] {
        let (text, info) = render(&engine.state, width, height);
        assert!(text.is_ascii(), "{text}");
        assert!(text.contains("container stop qtest"), "{text}");
        assert!(text.contains("waiting to quit"), "{text}");
        assert_eq!(
            keymap::map_key(
                &engine.state,
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
                &info
            ),
            [Command::ForceQuit]
        );
        assert!(
            keymap::map_key(
                &engine.state,
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT),
                &info
            )
            .is_empty()
        );
    }
    engine.dispatch(Command::CloseOverlay);
    assert!(!engine.state.quitting);
    assert!(!engine.state.quit);
    assert_eq!(engine.state.overlay, Overlay::None);
    engine.shutdown();
}

#[tokio::test]
async fn missing_cli_keeps_full_message_history_scrollable_and_ascii() {
    let (tx, _rx) = mpsc::channel(64);
    let mut engine = Engine::new(Client::new(Arc::new(MockRunner::new())), tx, true);
    engine.apply(AppEvent::VersionChecked(Err(
        bushel::client::CliError::CliMissing {
            raw: "container executable missing".into(),
        },
    )));
    assert_eq!(engine.state.screen, Screen::CliMissing);
    for i in 0..100 {
        engine.state.log_message(format!("message {i}"));
    }
    engine.dispatch(Command::OpenMessageLog);
    for (width, height) in [(55, 20), (120, 40)] {
        engine.state.message_scroll = 0;
        let (text, info) = render(&engine.state, width, height);
        assert!(text.is_ascii(), "{text}");
        assert!(text.contains("message 99"), "{text}");
        assert!(info.message_max_scroll > 0);
        let commands = keymap::map_key(
            &engine.state,
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE),
            &info,
        );
        assert_eq!(
            commands,
            [Command::SetMessageScroll(info.message_max_scroll)]
        );
        engine.dispatch(commands[0].clone());
        let (text, _) = render(&engine.state, width, height);
        assert!(text.contains("version check failed"), "{text}");
        assert_eq!(
            keymap::map_key(
                &engine.state,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &info
            ),
            [Command::ForceQuit]
        );
    }
    engine.shutdown();
}

#[test]
fn control_and_alt_keys_do_not_type_or_run_actions() {
    let mut state = AppState::new(true);
    for overlay in [
        Overlay::None,
        Overlay::PullInput {
            text: String::new(),
        },
    ] {
        state.overlay = overlay;
        for modifiers in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            assert!(
                keymap::map_key(
                    &state,
                    KeyEvent::new(KeyCode::Char('d'), modifiers),
                    &draw::DrawInfo::default()
                )
                .is_empty()
            );
        }
    }
}

#[test]
fn reduced_motion_disarms_animation_for_splash_and_service_start() {
    let mut state = AppState::new(true);
    state.config.reduced_motion = true;
    state.screen = Screen::Splash;
    state.service_starting = true;
    let ui = Ui::new(
        Theme {
            truecolor: false,
            ascii: true,
            reduced_motion: false,
        },
        true,
    );
    assert!(!ui.animating(&state));
    assert!(!ui.ambient_active());
}

#[test]
fn reduced_motion_does_not_arm_frames_for_a_pull() {
    let mut state = AppState::new(true);
    state.config.reduced_motion = true;
    state.pull = Some(bushel::engine::state::PullState {
        reference: "alpine".into(),
        lines: Vec::new(),
        started: std::time::Instant::now(),
    });
    let ui = Ui::new(Theme::detect(true), true);
    assert!(!ui.animating(&state));
    assert_eq!(ui.theme.spinner(0), ui.theme.spinner(7));
}

#[test]
fn input_interrupts_a_running_transition_and_reduced_motion_clears_it() {
    let mut state = AppState::new(true);
    let mut ui = Ui::new(
        Theme {
            truecolor: false,
            ascii: true,
            reduced_motion: false,
        },
        false,
    );
    let mut terminal = Terminal::new(TestBackend::new(55, 20)).unwrap();
    terminal
        .draw(|f| ui.render(f, &state, std::time::Duration::ZERO))
        .unwrap();
    state.focus = Focus::Detail;
    terminal
        .draw(|f| ui.render(f, &state, std::time::Duration::ZERO))
        .unwrap();
    assert!(ui.animating(&state));
    ui.interrupt();
    assert!(!ui.animating(&state));
    state.focus = Focus::List;
    terminal
        .draw(|f| ui.render(f, &state, std::time::Duration::ZERO))
        .unwrap();
    assert!(ui.animating(&state));
    state.config.reduced_motion = true;
    ui.sync_config(&state.config);
    assert!(!ui.animating(&state));
    assert!(!ui.ambient_active());
}

#[test]
fn renderer_proofs_cover_the_floor_and_wide_frames() {
    let destination = std::env::var_os("BUSHEL_RENDER_PROOF_DIR");
    let mut state = AppState::new(true);
    state.config.ascii = true;
    state.config.reduced_motion = true;
    let containers: Vec<ContainerJson> =
        serde_json::from_str(include_str!("../fixtures/1.2.0/ls.json")).unwrap();
    state.update_containers(&containers);
    state.log_owner = Some("qtest".into());
    for i in 0..10_000 {
        state.push_log_line(format!("log line {i}"));
    }
    for (width, height) in [(55, 20), (120, 40)] {
        for (name, overlay) in [
            ("following", Overlay::None),
            (
                "prompt",
                Overlay::PullInput {
                    text: format!("{}:latest", "registry/".repeat(30)),
                },
            ),
            ("messages", Overlay::MessageLog),
        ] {
            state.overlay = overlay;
            state.messages = (0..100)
                .map(|i| format!("message {i}: full stderr retained"))
                .collect();
            let (text, _) = render(&state, width, height);
            assert!(text.is_ascii(), "{text}");
            if name == "following" {
                assert!(text.contains("log line 9999"));
            }
            if name == "prompt" {
                assert!(text.contains(":latest_"));
            }
            if name == "messages" {
                assert!(text.contains("message 99"));
            }
            if let Some(destination) = &destination {
                let rows = text
                    .as_bytes()
                    .chunks(width as usize)
                    .map(|row| std::str::from_utf8(row).unwrap().trim_end())
                    .collect::<Vec<_>>()
                    .join("\n");
                std::fs::write(
                    std::path::Path::new(destination).join(format!("{name}-{width}x{height}.txt")),
                    rows,
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn the_floor_explains_detail_focus() {
    let mut state = AppState::new(true);
    state.focus = Focus::Detail;
    let (text, _) = render(&state, 55, 20);
    assert!(text.contains("j/k scroll"));
    assert!(text.contains("esc back"));
}

#[test]
fn every_part_of_a_long_command_preview_can_be_read() {
    let mut state = AppState::new(true);
    state.overlay = Overlay::Confirm {
        command: format!("container delete {}TAIL", "a".repeat(500)),
        action: bushel::engine::ActionKind::DeleteContainer,
        target: "test".into(),
    };
    for (width, height) in [(55, 20), (120, 40)] {
        let (top, info) = render(&state, width, height);
        assert!(top.contains("container delete"));
        assert!(info.confirm_max_scroll > 0);
        assert_eq!(
            keymap::map_key(
                &state,
                KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE),
                &info
            ),
            [Command::SetConfirmScroll(info.confirm_max_scroll)]
        );
        state.confirm_scroll = usize::MAX;
        let (bottom, _) = render(&state, width, height);
        assert!(bottom.contains("TAIL"));
        assert!(bottom.contains("[y] run"));
        state.confirm_scroll = 0;
    }
}

#[tokio::test]
async fn full_message_history_and_multiline_error_tails_are_reachable() {
    let (tx, _rx) = mpsc::channel(64);
    let mut engine = Engine::new(Client::new(Arc::new(MockRunner::new())), tx, true);
    for (width, height) in [(55, 20), (120, 40)] {
        for multiline in [false, true] {
            engine.state.messages.clear();
            let expected = if multiline {
                engine.state.log_message(format!(
                    "pull x failed:\n{}\nError: manifest unknown",
                    "noise\n".repeat(50)
                ));
                "Error: manifest unknown"
            } else {
                engine.state.log_message("boom: first");
                for i in 0..60 {
                    engine.state.log_message(format!("noise {i}"));
                }
                "boom: first"
            };
            engine.dispatch(Command::OpenMessageLog);
            assert_eq!(engine.state.message_scroll, 0);
            let (_, info) = render(&engine.state, width, height);
            engine.dispatch(Command::SetMessageScroll(info.message_max_scroll));
            assert!(render(&engine.state, width, height).0.contains(expected));
            let drawn = draw::DrawInfo {
                message_max_scroll: 4,
                ..Default::default()
            };
            engine.state.message_scroll = 0;
            assert_eq!(
                keymap::map_key(
                    &engine.state,
                    KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
                    &drawn
                ),
                [Command::SetMessageScroll(1)]
            );
            assert_eq!(
                keymap::map_key(
                    &engine.state,
                    KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE),
                    &drawn
                ),
                [Command::SetMessageScroll(4)]
            );
        }
    }
    engine.shutdown();
}

#[test]
fn service_start_progress_and_failure_are_visible_at_the_floor_and_wide() {
    let mut state = AppState::new(true);
    state.screen = Screen::ServiceDown;
    state.service_starting = true;
    state.service_output = (0..40).map(|i| format!("line {i}")).collect();
    state.toast("failed to spawn service start: x", true);
    for (width, height) in [(55, 20), (120, 40)] {
        let (text, _) = render(&state, width, height);
        assert!(text.contains("line 39"), "{text}");
        assert!(text.contains("failed to spawn service start: x"), "{text}");
        assert!(text.contains("[m] message log"));
    }
    state.overlay = Overlay::MessageLog;
    for code in [
        KeyCode::Esc,
        KeyCode::Char('m'),
        KeyCode::Char('q'),
        KeyCode::Char('?'),
    ] {
        assert_eq!(
            keymap::map_key(
                &state,
                KeyEvent::new(code, KeyModifiers::NONE),
                &Default::default()
            ),
            [Command::CloseOverlay]
        );
    }
}

#[tokio::test]
async fn service_loss_preserves_message_log_but_clears_other_overlays() {
    for overlay in [
        Overlay::MessageLog,
        Overlay::Help,
        Overlay::PullInput {
            text: "unfinished".into(),
        },
    ] {
        let (tx, _rx) = mpsc::channel(64);
        let mut engine = Engine::new(Client::new(Arc::new(MockRunner::new())), tx, true);
        engine.state.overlay = overlay.clone();
        engine.apply(AppEvent::ServiceProbe(Err(
            bushel::client::CliError::ServiceDown {
                raw: "service down".into(),
            },
        )));
        assert_eq!(engine.state.screen, Screen::ServiceDown);
        assert_eq!(
            engine.state.overlay,
            if overlay == Overlay::MessageLog {
                overlay
            } else {
                Overlay::None
            }
        );
        engine.shutdown();
    }
}

#[test]
fn focus_has_a_symbol_only_cue_in_both_layouts_and_sizes() {
    let mut state = AppState::new(true);
    for layout in [
        bushel::config::LayoutMode::Rail,
        bushel::config::LayoutMode::Table,
    ] {
        state.config.layout = layout;
        for (width, height) in [(55, 20), (120, 40)] {
            state.focus = Focus::List;
            let list = render(&state, width, height).0;
            state.focus = Focus::Detail;
            let detail = render(&state, width, height).0;
            let row = |s: String| {
                s.chars()
                    .skip(width as usize * (height as usize - 1))
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>()
            };
            assert_ne!(row(list), row(detail), "{layout:?} {width}x{height}");
        }
    }
}

#[test]
fn prompts_keep_the_last_ten_characters_visible_in_all_fields() {
    let text = format!("{}0123456789", "x".repeat(50));
    for overlay in [
        Overlay::PullInput { text: text.clone() },
        Overlay::TagInput { text: text.clone() },
        Overlay::CreateVolumeInput { text },
    ] {
        let mut state = AppState::new(true);
        state.overlay = overlay;
        for (width, height) in [(55, 20), (80, 24)] {
            assert!(render(&state, width, height).0.contains("0123456789_"));
        }
    }
}

#[test]
fn twenty_thousand_log_lines_keep_exactly_the_last_ten_thousand() {
    let mut state = AppState::new(true);
    for i in 0..20_000 {
        state.push_log_line(format!("line {i}"));
    }
    assert_eq!(state.log_lines.len(), bushel::engine::state::LOG_RING_CAP);
    assert_eq!(state.log_lines.front().unwrap(), "line 10000");
    assert_eq!(state.log_lines.back().unwrap(), "line 19999");
    let mut batch = AppState::new(true);
    batch.extend_log_lines((0..20_000).map(|i| format!("line {i}")));
    assert_eq!(batch.log_lines, state.log_lines);
}

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bushel::client::Client;
use bushel::engine::{AppEvent, Command, Engine, Overlay, Pane, Screen, UiAction};
use bushel::runner::{KillHandle, LineStream, MockRunner, Output, Runner};
use tokio::sync::mpsc;

struct DelayedRunner {
    mock: MockRunner,
}

#[async_trait]
impl Runner for DelayedRunner {
    async fn run(&self, args: &[String]) -> std::io::Result<Output> {
        let output = self.mock.run(args).await?;
        if args == ["stop", "qtest"] {
            tokio::time::sleep(Duration::from_millis(500)).await;
        } else if args == ["volume", "prune"] {
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
        Ok(output)
    }

    fn spawn_stream(&self, args: &[String]) -> std::io::Result<(LineStream, KillHandle)> {
        self.mock.spawn_stream(args)
    }
}

fn engine() -> (
    Engine<DelayedRunner>,
    mpsc::Receiver<AppEvent>,
    Arc<DelayedRunner>,
) {
    let mock = MockRunner::new();
    mock.on_default(Output::ok("[]"));
    let runner = Arc::new(DelayedRunner { mock });
    let (tx, rx) = mpsc::channel(1024);
    let mut engine = Engine::new(Client::new(runner.clone()), tx, true);
    engine.apply(AppEvent::Containers(
        1,
        Ok(serde_json::from_slice(include_bytes!("../fixtures/1.2.0/ls.json")).unwrap()),
    ));
    (engine, rx, runner)
}

#[tokio::test]
async fn quit_warns_before_interrupting_a_delayed_restart() {
    let (mut engine, _rx, _runner) = engine();
    engine.dispatch(Command::Run(UiAction::Restart));
    engine.dispatch(Command::Quit);
    assert!(
        !engine.state.quit,
        "quit must offer to wait for the restart"
    );
}

async fn completion(
    rx: &mut mpsc::Receiver<AppEvent>,
    engine: &mut Engine<DelayedRunner>,
) -> AppEvent {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        if matches!(
            event,
            AppEvent::ActionDone { .. } | AppEvent::PruneDone { .. }
        ) {
            return event;
        }
        engine.apply(event);
    }
}

#[tokio::test]
async fn waiting_preserves_both_restart_steps_and_exits_only_after_completion_is_applied() {
    let (mut engine, mut rx, runner) = engine();
    engine.dispatch(Command::Run(UiAction::Restart));
    engine.dispatch(Command::Quit);
    engine.dispatch(Command::WaitAndQuit);
    assert!(engine.state.quitting);
    // Commands that could start other mutations or leave the waiting overlay are refused.
    engine.dispatch(Command::SwitchPane(Pane::Volumes));
    engine.dispatch(Command::Run(UiAction::Prune));
    engine.dispatch(Command::StartService);
    engine.dispatch(Command::OpenHelp);
    assert_eq!(engine.state.pane, Pane::Containers);
    assert!(!engine.state.service_starting);
    assert!(matches!(engine.state.overlay, Overlay::QuitConfirm { .. }));

    let done = completion(&mut rx, &mut engine).await;
    assert!(
        !engine.state.quit,
        "receiving without applying must not quit"
    );
    let commands = runner.mock.commands();
    let stop = commands
        .iter()
        .position(|c| c == "container stop qtest")
        .unwrap();
    let start = commands
        .iter()
        .position(|c| c == "container start qtest")
        .unwrap();
    assert!(stop < start);
    assert!(
        !commands
            .iter()
            .any(|c| c.contains("prune") || c.contains("system start"))
    );
    engine.apply(done);
    assert!(engine.state.quit);
    // Awaiting a confirming poll does not count as an interrupted child command.
    assert!(engine.quit_blocked_by().is_empty());
    assert!(engine.shutdown().is_empty());
}

#[tokio::test]
async fn forced_quit_reports_a_delayed_restart() {
    let (mut engine, _rx, _runner) = engine();
    engine.dispatch(Command::Run(UiAction::Restart));
    engine.dispatch(Command::ForceQuit);
    assert!(engine.state.quit);
    assert_eq!(
        engine.shutdown(),
        ["container stop qtest && container start qtest"]
    );
}

#[tokio::test]
async fn waiting_requires_completion_of_each_action_and_prune() {
    let (mut engine, mut rx, _runner) = engine();
    engine.dispatch(Command::Run(UiAction::Restart));
    engine.dispatch(Command::SwitchPane(Pane::Volumes));
    engine.dispatch(Command::Run(UiAction::Prune));
    engine.dispatch(Command::ConfirmYes);
    engine.dispatch(Command::Quit);
    assert_eq!(
        engine.quit_blocked_by(),
        [
            "container stop qtest && container start qtest",
            "container volume prune"
        ]
    );
    engine.dispatch(Command::WaitAndQuit);
    let first = completion(&mut rx, &mut engine).await;
    engine.apply(first);
    assert!(!engine.state.quit);
    assert_eq!(engine.quit_blocked_by().len(), 1);
    let second = completion(&mut rx, &mut engine).await;
    engine.apply(second);
    assert!(engine.state.quit);
    assert!(engine.shutdown().is_empty());
}

fn start_pull_and_service(engine: &mut Engine<DelayedRunner>) {
    engine.dispatch(Command::SwitchPane(Pane::Images));
    engine.dispatch(Command::Run(UiAction::Pull));
    for c in "alpine".chars() {
        engine.dispatch(Command::OverlayChar(c));
    }
    engine.dispatch(Command::OverlaySubmit);
    engine.dispatch(Command::StartService);
}

#[tokio::test]
async fn quit_warns_and_waits_for_pull_and_service_start() {
    let (mut engine, _rx, _runner) = engine();
    start_pull_and_service(&mut engine);
    engine.state.screen = Screen::ServiceDown;
    engine.dispatch(Command::Quit);
    assert_eq!(
        engine.quit_blocked_by(),
        [
            "container image pull alpine:latest --progress plain",
            "container system start --enable-kernel-install"
        ]
    );
    assert!(!engine.state.quit);
    engine.dispatch(Command::WaitAndQuit);
    engine.apply(AppEvent::PullDone {
        reference: "alpine:latest".into(),
        code: 0,
    });
    assert!(!engine.state.quit);
    engine.apply(AppEvent::ServiceStartExited(0));
    assert!(engine.state.quit);
    assert!(engine.shutdown().is_empty());
}

#[tokio::test]
async fn forced_quit_reports_pull_and_service_commands() {
    let (mut engine, _rx, _runner) = engine();
    start_pull_and_service(&mut engine);
    engine.dispatch(Command::ForceQuit);
    assert_eq!(
        engine.shutdown(),
        [
            "container image pull alpine:latest --progress plain",
            "container system start --enable-kernel-install"
        ]
    );
}

#[tokio::test]
async fn cancelling_wait_keeps_the_app_open_after_the_action_finishes() {
    let (mut engine, mut rx, _runner) = engine();
    engine.dispatch(Command::Run(UiAction::Restart));
    engine.dispatch(Command::Quit);
    engine.dispatch(Command::WaitAndQuit);
    engine.dispatch(Command::CloseOverlay);
    assert!(!engine.state.quitting);
    assert_eq!(engine.state.overlay, Overlay::None);
    let done = completion(&mut rx, &mut engine).await;
    engine.apply(done);
    assert!(!engine.state.quit);
    engine.dispatch(Command::Quit);
    assert!(engine.state.quit);
}

#[test]
fn quit_keys_preserve_the_immediate_ctrl_c_escape_on_every_screen() {
    use bushel::ui::draw::DrawInfo;
    use bushel::ui::keymap::map_key;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    for screen in [Screen::Splash, Screen::ServiceDown, Screen::Main] {
        let mut state = bushel::engine::AppState::new(true);
        state.screen = screen;
        assert_eq!(
            map_key(
                &state,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &DrawInfo::default()
            ),
            [Command::ForceQuit]
        );
        state.overlay = Overlay::QuitConfirm {
            commands: vec!["container stop qtest".into()],
            scroll: 0,
        };
        for (key, command) in [('q', Command::ForceQuit), ('w', Command::WaitAndQuit)] {
            assert_eq!(
                map_key(
                    &state,
                    KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE),
                    &DrawInfo::default()
                ),
                [command]
            );
        }
        assert_eq!(
            map_key(
                &state,
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
                &DrawInfo::default()
            ),
            []
        );
    }
}

#[test]
fn quit_overlay_shows_commands_and_choices_on_main_and_service_screens() {
    use bushel::ui::{draw, theme::Theme};
    use ratatui::{Terminal, backend::TestBackend};
    for screen in [Screen::Main, Screen::ServiceDown] {
        let mut state = bushel::engine::AppState::new(true);
        state.screen = screen;
        state.overlay = Overlay::QuitConfirm {
            commands: vec!["container stop qtest && container start qtest".into()],
            scroll: 0,
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 22)).unwrap();
        let theme = Theme {
            truecolor: false,
            ascii: true,
        };
        terminal
            .draw(|frame| {
                draw::draw(frame, &state, &theme);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("container stop qtest && container start qtest"));
        assert!(rendered.contains("[w] wait and quit"));
        assert!(rendered.contains("[q] quit now"));
    }
}

#[test]
fn quit_command_scroll_is_bounded_and_can_reach_every_command() {
    use bushel::ui::{draw, keymap::map_key, theme::Theme};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};
    let mut state = bushel::engine::AppState::new(true);
    state.overlay = Overlay::QuitConfirm {
        commands: (0..40)
            .map(|n| format!("container stop qtest-{n}"))
            .collect(),
        scroll: 0,
    };
    let mut terminal = Terminal::new(TestBackend::new(55, 20)).unwrap();
    let theme = Theme {
        truecolor: false,
        ascii: true,
    };
    let mut info = draw::DrawInfo::default();
    terminal
        .draw(|frame| {
            info = draw::draw(frame, &state, &theme);
        })
        .unwrap();
    assert!(info.quit_max_scroll > 0);
    if let Overlay::QuitConfirm { scroll, .. } = &mut state.overlay {
        *scroll = info.quit_max_scroll;
    }
    assert_eq!(
        map_key(
            &state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &info
        ),
        [Command::SetQuitScroll(info.quit_max_scroll)]
    );
    assert_eq!(
        map_key(
            &state,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &info
        ),
        [Command::SetQuitScroll(info.quit_max_scroll - 1)]
    );
    terminal
        .draw(|frame| {
            draw::draw(frame, &state, &theme);
        })
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(rendered.contains("container stop qtest-39"));
    for (width, height) in [(1, 1), (20, 8), (40, 10)] {
        let mut tiny = Terminal::new(TestBackend::new(width, height)).unwrap();
        tiny.draw(|frame| {
            draw::draw(frame, &state, &theme);
        })
        .unwrap();
    }
}

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use async_trait::async_trait;
use bushel::client::Client;
use bushel::engine::{AppEvent, Command, Engine, UiAction};
use bushel::runner::{KillHandle, LineStream, MockRunner, Output, Runner};
use tokio::sync::mpsc;

struct RestartRunner {
    mock: MockRunner,
    stopped: AtomicBool,
    stop_fails: bool,
    list_delay_ms: AtomicU64,
    list_fails: AtomicBool,
}

impl RestartRunner {
    fn new(stop_fails: bool) -> Self {
        let mock = MockRunner::new();
        mock.on_default(Output::ok("[]"));
        mock.on(&["--version"], Output::ok("container CLI version 1.2.0"));
        mock.on(
            &["system", "status", "--format", "json"],
            Output::ok(std::fs::read("fixtures/1.2.0/system_status_running.json").unwrap()),
        );
        Self {
            mock,
            stopped: AtomicBool::new(false),
            stop_fails,
            list_delay_ms: AtomicU64::new(0),
            list_fails: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Runner for RestartRunner {
    async fn run(&self, args: &[String]) -> std::io::Result<Output> {
        match args.first().map(String::as_str) {
            Some("stop") => {
                // A lost CLI response does not undo a stop that already took effect.
                self.stopped.store(true, Ordering::SeqCst);
                Ok(if self.stop_fails {
                    Output::fail(1, "Error: stop response lost")
                } else {
                    Output::ok("qtest")
                })
            }
            Some("start") => Ok(Output::fail(1, "Error: start failed")),
            Some("ls") => {
                let delay = self.list_delay_ms.load(Ordering::SeqCst);
                if delay > 0 {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                if self.list_fails.load(Ordering::SeqCst) {
                    return Ok(Output::fail(1, "Error: list failed"));
                }
                let mut rows = std::fs::read_to_string("fixtures/1.2.0/ls.json").unwrap();
                if self.stopped.load(Ordering::SeqCst) {
                    rows = rows.replace(r#""state":"running""#, r#""state":"stopped""#);
                }
                Ok(Output::ok(rows))
            }
            _ => self.mock.run(args).await,
        }
    }

    fn spawn_stream(&self, args: &[String]) -> std::io::Result<(LineStream, KillHandle)> {
        self.mock.spawn_stream(args)
    }
}

async fn settle(engine: &mut Engine<RestartRunner>, rx: &mut mpsc::Receiver<AppEvent>) {
    for _ in 0..30 {
        tokio::task::yield_now().await;
        while let Ok(event) = rx.try_recv() {
            engine.apply(event);
        }
    }
}

async fn started_engine(
    stop_fails: bool,
) -> (
    Engine<RestartRunner>,
    mpsc::Receiver<AppEvent>,
    Arc<RestartRunner>,
) {
    let runner = Arc::new(RestartRunner::new(stop_fails));
    let (tx, mut rx) = mpsc::channel(128);
    let mut engine = Engine::new(Client::new(runner.clone()), tx, true);
    engine.start();
    settle(&mut engine, &mut rx).await;
    (engine, rx, runner)
}

async fn failed_restart(
    stop_fails: bool,
) -> (
    Engine<RestartRunner>,
    mpsc::Receiver<AppEvent>,
    Arc<RestartRunner>,
) {
    let (mut engine, mut rx, runner) = started_engine(stop_fails).await;
    engine.dispatch(Command::Run(UiAction::Restart));
    settle(&mut engine, &mut rx).await;
    assert!(engine.state.toast.as_ref().unwrap().error);
    assert!(engine.state.containers[0].pending.is_none());
    (engine, rx, runner)
}

async fn take_event(
    engine: &mut Engine<RestartRunner>,
    rx: &mut mpsc::Receiver<AppEvent>,
    wanted: impl Fn(&AppEvent) -> bool,
) -> AppEvent {
    loop {
        let event = rx.recv().await.expect("Engine sender remains alive");
        if wanted(&event) {
            return event;
        }
        engine.apply(event);
    }
}

fn announced_external_stop(engine: &Engine<RestartRunner>) -> bool {
    engine
        .state
        .messages
        .iter()
        .any(|message| message.contains("qtest stopped externally"))
}

#[tokio::test(start_paused = true)]
async fn first_post_completion_poll_retains_attribution_past_the_completion_window() {
    let (mut engine, mut rx, runner) = failed_restart(false).await;
    tokio::time::advance(Duration::from_secs(1)).await;
    runner.list_delay_ms.store(9_500, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    tokio::time::advance(Duration::from_secs(9)).await;
    engine.on_tick(); // Completion window ends while the relevant poll is still running.
    settle(&mut engine, &mut rx).await;
    tokio::time::advance(Duration::from_millis(500)).await;
    settle(&mut engine, &mut rx).await;
    assert!(!engine.state.containers[0].is_running());
    assert!(!announced_external_stop(&engine));
    assert!(engine.state.toast.as_ref().unwrap().error);
    // The protected success consumes attribution; a later stop is external.
    runner.list_delay_ms.store(0, Ordering::SeqCst);
    runner.stopped.store(false, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    runner.stopped.store(true, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    assert!(announced_external_stop(&engine));
    engine.shutdown();
}

#[tokio::test(start_paused = true)]
async fn failed_first_restart_step_still_attributes_its_observed_stop() {
    let (mut engine, mut rx, _) = failed_restart(true).await;
    assert!(
        engine
            .state
            .toast
            .as_ref()
            .unwrap()
            .text
            .contains("stop failed")
    );
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    assert!(!engine.state.containers[0].is_running());
    assert!(!announced_external_stop(&engine));
    assert!(engine.state.toast.as_ref().unwrap().error);
    engine.shutdown();
}

#[tokio::test(start_paused = true)]
async fn a_pre_completion_poll_queued_past_expiry_keeps_its_stop_attribution() {
    let (mut engine, mut rx, runner) = started_engine(false).await;
    engine.dispatch(Command::Run(UiAction::Restart));
    let done = take_event(&mut engine, &mut rx, |event| {
        matches!(event, AppEvent::ActionDone { .. })
    })
    .await;
    engine.on_tick();
    let earlier_poll = take_event(&mut engine, &mut rx, |event| {
        matches!(event, AppEvent::Containers(..))
    })
    .await;
    engine.apply(done);
    // Interactive exec can prevent the main loop from applying an already
    // queued poll, even when the CLI returned within its read timeout.
    tokio::time::advance(Duration::from_millis(10_500)).await;
    engine.apply(earlier_poll);
    assert!(!engine.state.containers[0].is_running());
    assert!(!announced_external_stop(&engine));
    runner.stopped.store(false, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    runner.stopped.store(true, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    assert!(announced_external_stop(&engine));
    engine.shutdown();
}

#[tokio::test(start_paused = true)]
async fn a_poll_started_after_the_window_does_not_hide_an_external_stop() {
    let (mut engine, mut rx, _) = failed_restart(false).await;
    tokio::time::advance(Duration::from_millis(10_500)).await;
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    assert!(announced_external_stop(&engine));
    engine.shutdown();
}

#[tokio::test(start_paused = true)]
async fn a_failed_protected_poll_does_not_extend_attribution_to_later_polls() {
    let (mut engine, mut rx, runner) = failed_restart(false).await;
    tokio::time::advance(Duration::from_secs(1)).await;
    runner.list_delay_ms.store(9_500, Ordering::SeqCst);
    runner.list_fails.store(true, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    tokio::time::advance(Duration::from_millis(9_500)).await;
    settle(&mut engine, &mut rx).await;
    runner.list_delay_ms.store(0, Ordering::SeqCst);
    runner.list_fails.store(false, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    assert!(announced_external_stop(&engine));
    engine.shutdown();
}

#[tokio::test(start_paused = true)]
async fn a_timed_out_protected_poll_does_not_extend_attribution_to_later_polls() {
    let (mut engine, mut rx, runner) = failed_restart(false).await;
    tokio::time::advance(Duration::from_secs(1)).await;
    runner.list_delay_ms.store(20_000, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    tokio::time::advance(Duration::from_secs(10)).await;
    settle(&mut engine, &mut rx).await;
    runner.list_delay_ms.store(0, Ordering::SeqCst);
    engine.on_tick();
    settle(&mut engine, &mut rx).await;
    assert!(announced_external_stop(&engine));
    engine.shutdown();
}

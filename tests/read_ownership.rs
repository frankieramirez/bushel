use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bushel::client::{
    Client,
    model::{ContainerJson, ImageJson, NetworkJson, StatsJson},
};
use bushel::engine::pending::Target;
use bushel::engine::{AppEvent, Command, Engine, Pane, UiAction};
use bushel::runner::{KillHandle, LineStream, MockRunner, Output, Runner, StreamEvent};
use tokio::sync::mpsc;

fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_slice(&std::fs::read(format!("fixtures/1.2.0/{name}")).unwrap()).unwrap()
}

struct DelayedRunner {
    mock: MockRunner,
    delay: &'static str,
    active: AtomicUsize,
    peak: AtomicUsize,
    started: AtomicUsize,
    cancelled: AtomicUsize,
}

struct ActiveRead<'a> {
    runner: &'a DelayedRunner,
    finished: bool,
}
impl Drop for ActiveRead<'_> {
    fn drop(&mut self) {
        self.runner.active.fetch_sub(1, Ordering::SeqCst);
        if !self.finished {
            self.runner.cancelled.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl DelayedRunner {
    fn new(delay: &'static str) -> Self {
        let mock = MockRunner::new();
        mock.on_default(Output::ok("[]"));
        mock.on(
            &["ls", "-a", "--format", "json"],
            Output::ok(std::fs::read("fixtures/1.2.0/ls.json").unwrap()),
        );
        Self {
            mock,
            delay,
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            started: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Runner for DelayedRunner {
    async fn run(&self, args: &[String]) -> std::io::Result<Output> {
        let delayed = match self.delay {
            "logs" => args.starts_with(&["logs".into(), "-n".into()]),
            "inspect" => {
                args.first().is_some_and(|arg| arg == "inspect")
                    || args.get(1).is_some_and(|arg| arg == "inspect")
            }
            "stats" => args.first().is_some_and(|arg| arg == "stats"),
            _ => false,
        };
        if delayed {
            self.started.fetch_add(1, Ordering::SeqCst);
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(active, Ordering::SeqCst);
            let mut guard = ActiveRead {
                runner: self,
                finished: false,
            };
            tokio::time::sleep(Duration::from_secs(3)).await;
            guard.finished = true;
        }
        if args.starts_with(&["logs".into(), "-n".into()]) {
            return Ok(Output::ok(format!("backlog {}", args.last().unwrap())));
        }
        self.mock.run(args).await
    }

    fn spawn_stream(&self, _args: &[String]) -> std::io::Result<(LineStream, KillHandle)> {
        let (tx, rx) = mpsc::channel(16);
        tx.try_send(StreamEvent::Stdout("live tail".into()))
            .unwrap();
        Ok((
            rx,
            KillHandle::new(move || {
                let _ = tx.try_send(StreamEvent::Exit(-1));
            }),
        ))
    }
}

fn setup(runner: Arc<DelayedRunner>) -> (Engine<DelayedRunner>, mpsc::Receiver<AppEvent>) {
    let (tx, rx) = mpsc::channel(128);
    (Engine::new(Client::new(runner), tx, true), rx)
}

async fn settle<R: Runner>(engine: &mut Engine<R>, rx: &mut mpsc::Receiver<AppEvent>) {
    for _ in 0..30 {
        tokio::task::yield_now().await;
        while let Ok(event) = rx.try_recv() {
            engine.apply(event);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn moving_selection_cancels_previous_backlogs_before_they_complete() {
    let runner = Arc::new(DelayedRunner::new("logs"));
    let (mut engine, mut rx) = setup(runner.clone());
    let base: Vec<ContainerJson> = fixture("ls.json");
    let rows = ["a", "b", "c"].map(|id| {
        let mut row = base[0].clone();
        row.id = id.into();
        row
    });
    engine.apply(AppEvent::Containers(1, Ok(rows.to_vec())));
    settle(&mut engine, &mut rx).await;
    engine.dispatch(Command::Move(1));
    settle(&mut engine, &mut rx).await;
    engine.dispatch(Command::Move(1));
    settle(&mut engine, &mut rx).await;
    assert_eq!(runner.started.load(Ordering::SeqCst), 3);
    assert_eq!(runner.cancelled.load(Ordering::SeqCst), 2);
    tokio::time::advance(Duration::from_secs(3)).await;
    settle(&mut engine, &mut rx).await;
    assert_eq!(engine.state.log_owner.as_deref(), Some("c"));
    assert_eq!(engine.state.log_lines, ["backlog c", "live tail"]);
    engine.shutdown();
}

#[tokio::test(start_paused = true)]
async fn moving_across_uncached_images_keeps_only_one_inspect_running() {
    let runner = Arc::new(DelayedRunner::new("inspect"));
    let (mut engine, mut rx) = setup(runner.clone());
    let rows: Vec<ImageJson> = serde_json::from_str(
        r#"[
        {"id":"a","configuration":{"name":"a"}}, {"id":"b","configuration":{"name":"b"}},
        {"id":"c","configuration":{"name":"c"}}, {"id":"d","configuration":{"name":"d"}},
        {"id":"e","configuration":{"name":"e"}}
    ]"#,
    )
    .unwrap();
    engine.apply(AppEvent::Images(1, Ok(rows)));
    engine.dispatch(Command::SwitchPane(Pane::Images));
    settle(&mut engine, &mut rx).await;
    for _ in 0..4 {
        engine.dispatch(Command::Move(1));
        settle(&mut engine, &mut rx).await;
    }
    assert_eq!(runner.started.load(Ordering::SeqCst), 5);
    assert_eq!(runner.cancelled.load(Ordering::SeqCst), 4);
    assert_eq!(runner.peak.load(Ordering::SeqCst), 1);
    tokio::time::advance(Duration::from_secs(3)).await;
    settle(&mut engine, &mut rx).await;
    assert_eq!(engine.state.inspect_cache.len(), 1);
    assert!(
        engine
            .state
            .inspect_cache
            .contains_key(&Target::new(Pane::Images, "e"))
    );
    engine.shutdown();
}

#[tokio::test(start_paused = true)]
async fn slow_stats_never_overlap_and_can_resume_after_completion() {
    let runner = Arc::new(DelayedRunner::new("stats"));
    let (mut engine, mut rx) = setup(runner.clone());
    engine.apply(AppEvent::Containers(1, Ok(fixture("ls.json"))));
    settle(&mut engine, &mut rx).await;
    for _ in 0..5 {
        engine.on_tick();
        settle(&mut engine, &mut rx).await;
        tokio::time::advance(Duration::from_secs(1)).await;
    }
    settle(&mut engine, &mut rx).await;
    assert_eq!(runner.peak.load(Ordering::SeqCst), 1);
    assert_eq!(runner.started.load(Ordering::SeqCst), 2);
    engine.shutdown();
}

#[tokio::test]
async fn old_follower_exit_queued_during_exec_cannot_end_the_new_follower() {
    let runner = Arc::new(DelayedRunner::new("none"));
    let (mut engine, mut rx) = setup(runner);
    engine.apply(AppEvent::Containers(1, Ok(fixture("ls.json"))));
    settle(&mut engine, &mut rx).await;
    engine.dispatch(Command::Run(UiAction::Exec));
    assert_eq!(engine.prepare_exec().join(" "), "exec -it qtest /bin/sh");
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    engine.after_exec();
    settle(&mut engine, &mut rx).await;
    assert_eq!(engine.follower_id(), Some("qtest"));
    assert!(!engine.state.follow_ended);
    engine.shutdown();
}

#[tokio::test]
async fn older_stats_samples_cannot_change_rates_or_the_next_baseline() {
    let (mut engine, _) = setup(Arc::new(DelayedRunner::new("none")));
    engine
        .state
        .update_containers(&fixture::<Vec<ContainerJson>>("ls.json"));
    let mut sample: Vec<StatsJson> = fixture("stats.json");
    let start = Instant::now();
    engine.apply(AppEvent::Stats {
        sequence: 10,
        taken_at: start,
        result: Ok(sample.clone()),
    });
    sample[0].cpu_usage_usec += 100_000;
    engine.apply(AppEvent::Stats {
        sequence: 11,
        taken_at: start + Duration::from_secs(1),
        result: Ok(sample.clone()),
    });
    assert_eq!(engine.state.containers[0].cpu_percent, Some(10.0));
    sample[0].cpu_usage_usec -= 50_000;
    engine.apply(AppEvent::Stats {
        sequence: 10,
        taken_at: start,
        result: Ok(sample.clone()),
    });
    assert_eq!(engine.state.containers[0].cpu_percent, Some(10.0));
    sample[0].cpu_usage_usec += 150_000;
    engine.apply(AppEvent::Stats {
        sequence: 12,
        taken_at: start + Duration::from_secs(2),
        result: Ok(sample),
    });
    assert_eq!(engine.state.containers[0].cpu_percent, Some(10.0));
}

#[tokio::test]
async fn older_network_polls_cannot_replace_newer_lists() {
    let (mut engine, _) = setup(Arc::new(DelayedRunner::new("none")));
    let rows: Vec<NetworkJson> = fixture("network_ls.json");
    engine.apply(AppEvent::Networks(2, Ok(rows)));
    engine.apply(AppEvent::Networks(1, Ok(vec![])));
    assert!(!engine.state.networks.is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_selected_inspect_invalidated_mid_read_cannot_repopulate_stale_json() {
    let runner = Arc::new(DelayedRunner::new("inspect"));
    let (mut engine, mut rx) = setup(runner.clone());
    let rows: Vec<ImageJson> = serde_json::from_str(
        r#"[{"id":"a","configuration":{"name":"a","descriptor":{"digest":"sha256:a"}}}]"#,
    )
    .unwrap();
    engine.apply(AppEvent::Images(1, Ok(rows.clone())));
    engine.dispatch(Command::SwitchPane(Pane::Images));
    settle(&mut engine, &mut rx).await;
    let mut newer = rows;
    newer[0].configuration.descriptor.as_mut().unwrap().digest = Some("sha256:b".into());
    runner.mock.set(
        &["image", "inspect", "a"],
        Output::ok(r#"{"digest":"sha256:b"}"#),
    );
    engine.apply(AppEvent::Images(2, Ok(newer)));
    settle(&mut engine, &mut rx).await;
    assert_eq!(runner.cancelled.load(Ordering::SeqCst), 1);
    tokio::time::advance(Duration::from_secs(3)).await;
    settle(&mut engine, &mut rx).await;
    let inspect = engine
        .state
        .inspect_cache
        .get(&Target::new(Pane::Images, "a"))
        .unwrap();
    assert!(inspect.json.contains("sha256:b"));
    assert_eq!(inspect.identity.as_deref(), Some("sha256:b"));
    engine.shutdown();
}

#[tokio::test]
async fn an_inspect_result_queued_before_invalidation_cannot_restore_stale_json() {
    let runner = Arc::new(DelayedRunner::new("none"));
    runner.mock.set(
        &["image", "inspect", "a"],
        Output::ok(r#"{"digest":"old"}"#),
    );
    let (mut engine, mut rx) = setup(runner.clone());
    let rows: Vec<ImageJson> = serde_json::from_str(
        r#"[{"id":"a","configuration":{"name":"a","descriptor":{"digest":"sha256:a"}}}]"#,
    )
    .unwrap();
    engine.apply(AppEvent::Images(1, Ok(rows.clone())));
    engine.dispatch(Command::SwitchPane(Pane::Images));
    let old = loop {
        let event = rx.recv().await.unwrap();
        if matches!(event, AppEvent::InspectLoaded { .. }) {
            break event;
        }
        engine.apply(event);
    };
    let mut newer = rows;
    newer[0].configuration.descriptor.as_mut().unwrap().digest = Some("sha256:b".into());
    runner.mock.set(
        &["image", "inspect", "a"],
        Output::ok(r#"{"digest":"new"}"#),
    );
    engine.apply(AppEvent::Images(2, Ok(newer)));
    engine.apply(old);
    assert!(
        !engine
            .state
            .inspect_cache
            .contains_key(&Target::new(Pane::Images, "a"))
    );
    settle(&mut engine, &mut rx).await;
    assert!(
        engine.state.inspect_cache[&Target::new(Pane::Images, "a")]
            .json
            .contains("new")
    );
    engine.shutdown();
}

#[tokio::test(start_paused = true)]
async fn exec_and_shutdown_cancel_backlog_and_inspect_reads() {
    for read in ["logs", "inspect"] {
        let runner = Arc::new(DelayedRunner::new(read));
        let (mut engine, mut rx) = setup(runner.clone());
        engine.apply(AppEvent::Containers(1, Ok(fixture("ls.json"))));
        if read == "inspect" {
            engine.dispatch(Command::SetDetailTab(bushel::engine::DetailTab::Inspect));
        }
        settle(&mut engine, &mut rx).await;
        assert_eq!(runner.active.load(Ordering::SeqCst), 1);
        engine.dispatch(Command::Run(UiAction::Exec));
        engine.prepare_exec();
        settle(&mut engine, &mut rx).await;
        assert_eq!(runner.cancelled.load(Ordering::SeqCst), 1);
        assert_eq!(runner.active.load(Ordering::SeqCst), 0);
        engine.after_exec();
        settle(&mut engine, &mut rx).await;
        assert_eq!(runner.active.load(Ordering::SeqCst), 1);
        engine.shutdown();
        settle(&mut engine, &mut rx).await;
        assert_eq!(runner.cancelled.load(Ordering::SeqCst), 2);
        assert_eq!(runner.active.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn an_older_list_success_cannot_clear_a_newer_poll_failure() {
    let (mut engine, _) = setup(Arc::new(DelayedRunner::new("none")));
    let original: Vec<ImageJson> = serde_json::from_str(
        r#"[{"id":"a","configuration":{"name":"a","descriptor":{"digest":"sha256:a"}}}]"#,
    )
    .unwrap();
    engine.apply(AppEvent::Images(10, Ok(original.clone())));
    engine.apply(AppEvent::Images(12, Err(bushel::client::CliError::Timeout)));
    let mut older = original;
    older[0].configuration.descriptor.as_mut().unwrap().digest = Some("sha256:stale".into());
    engine.apply(AppEvent::Images(11, Ok(older)));
    assert_eq!(engine.state.images[0].digest.as_deref(), Some("sha256:a"));
    assert!(matches!(
        engine.state.reads[Pane::Images.index()],
        bushel::engine::ReadStatus::Failed { .. }
    ));
}

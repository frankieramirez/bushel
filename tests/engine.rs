//! Headless Engine tests: a MockRunner replays captured fixtures, commands are
//! dispatched as the UI would, and state is asserted — no terminal involved.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bushel::client::Client;
use bushel::engine::{
    ActionKind, AppEvent, AppState, Command, DetailTab, Engine, Overlay, Pane, PendingPhase,
    Screen, UiAction,
};
use bushel::runner::{MockRunner, Output, StreamEvent};
use tokio::sync::mpsc;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("fixtures/1.2.0/{name}")).unwrap()
}

fn fixture_str(name: &str) -> String {
    String::from_utf8(fixture(name)).unwrap()
}

/// The standard happy-path mock: service up, one running + one stopped container,
/// two images, two volumes (qvol in use by old-batch), quiet logs.
fn happy_mock() -> MockRunner {
    let mock = MockRunner::new();
    mock.on(
        &["--version"],
        Output::ok("container CLI version 1.2.0 (build: release, commit: 6e65319)\n"),
    );
    mock.on(
        &["system", "status", "--format", "json"],
        Output::ok(fixture("system_status_running.json")),
    );
    mock.on(
        &["ls", "-a", "--format", "json"],
        Output::ok(fixture("ls.json")),
    );
    mock.on(
        &["image", "ls", "--format", "json"],
        Output::ok(fixture("image_ls.json")),
    );
    mock.on(
        &["volume", "ls", "--format", "json"],
        Output::ok(fixture("volume_ls.json")),
    );
    mock.on(
        &["stats", "--no-stream", "--format", "json"],
        Output::ok(fixture("stats.json")),
    );
    mock.on(
        &["logs", "-n", "200", "qtest"],
        Output::ok("backlog-1\nbacklog-2\n"),
    );
    mock.on_stream(
        &["logs", "-f", "qtest"],
        vec![StreamEvent::Stdout("follow-1".into())],
    );
    mock
}

struct Harness {
    engine: Engine<MockRunner>,
    rx: mpsc::Receiver<AppEvent>,
    mock: Arc<MockRunner>,
}

impl Harness {
    fn new(mock: MockRunner) -> Self {
        let mock = Arc::new(mock);
        let client = Client::new(Arc::clone(&mock));
        let (tx, rx) = mpsc::channel(1024);
        let engine = Engine::new(client, tx, true);
        Self { engine, rx, mock }
    }

    fn started(mock: MockRunner) -> Self {
        let mut h = Self::new(mock);
        h.engine.start();
        h.pump();
        h
    }

    /// Apply every event the spawned tasks produce until the channel goes quiet.
    fn pump(&mut self) {
        let rt = tokio::runtime::Handle::current();
        loop {
            let next = rt.block_on(async {
                tokio::time::timeout(Duration::from_millis(100), self.rx.recv()).await
            });
            match next {
                Ok(Some(ev)) => self.engine.apply(ev),
                _ => break,
            }
        }
    }

    fn state(&self) -> &AppState {
        &self.engine.state
    }
}

// pump() blocks in place, so tests need the multi-thread runtime.
macro_rules! engine_test {
    ($name:ident, $body:expr) => {
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn $name() {
            tokio::task::block_in_place($body);
        }
    };
}

engine_test!(startup_populates_all_three_panes_from_fixtures, || {
    let h = Harness::started(happy_mock());
    let s = h.state();

    assert_eq!(s.screen, Screen::Main);
    // running-first then alphabetical
    let ids: Vec<&str> = s.containers.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["qtest", "old-batch"]);
    assert!(s.containers[0].is_running());
    assert_eq!(s.images.len(), 2);
    assert_eq!(s.volumes.len(), 2);
    assert_eq!(s.cli_version.as_deref(), Some("1.2.0"));
    assert!(s.version_banner.is_none());
    // first container selected
    assert_eq!(s.selected[0].as_deref(), Some("qtest"));
});

engine_test!(volume_in_use_badge_derives_from_container_mounts, || {
    let h = Harness::started(happy_mock());
    let qvol = h.state().volumes.iter().find(|v| v.name == "qvol").unwrap();
    assert_eq!(qvol.in_use_by, vec!["old-batch"]);
    let scratch = h
        .state()
        .volumes
        .iter()
        .find(|v| v.name == "scratch")
        .unwrap();
    assert!(!scratch.in_use());
});

engine_test!(untested_cli_version_raises_a_dismissible_banner, || {
    let mock = happy_mock();
    mock.set(
        &["--version"],
        Output::ok("container CLI version 1.3.0 (build: release, commit: abc)\n"),
    );
    let mut h = Harness::started(mock);

    let banner = h.state().version_banner.clone().expect("banner");
    assert!(
        banner.contains("1.3.0") && banner.contains("1.2.x"),
        "{banner}"
    );

    h.engine.dispatch(Command::DismissBanner);
    assert!(h.state().version_banner.is_none());
});

engine_test!(service_down_at_startup_takes_over_the_screen, || {
    let mock = MockRunner::new();
    mock.on(&["--version"], Output::ok("container CLI version 1.2.0\n"));
    mock.on(
        &["system", "status", "--format", "json"],
        Output {
            code: 1,
            stdout: fixture("system_status_down.json"),
            stderr: Vec::new(),
        },
    );
    mock.on_default(Output::fail(1, fixture("stderr/service_down_ls.txt")));
    let h = Harness::started(mock);

    assert_eq!(h.state().screen, Screen::ServiceDown);
});

engine_test!(service_recovery_returns_to_main_and_repolls, || {
    let mock = MockRunner::new();
    mock.on(&["--version"], Output::ok("container CLI version 1.2.0\n"));
    // first probe: down; second probe: running
    mock.on(
        &["system", "status", "--format", "json"],
        Output {
            code: 1,
            stdout: fixture("system_status_down.json"),
            stderr: Vec::new(),
        },
    );
    mock.on(
        &["system", "status", "--format", "json"],
        Output::ok(fixture("system_status_running.json")),
    );
    // lists fail first (XPC), succeed after recovery
    mock.on(
        &["ls", "-a", "--format", "json"],
        Output::fail(1, fixture("stderr/service_down_ls.txt")),
    );
    mock.on(
        &["ls", "-a", "--format", "json"],
        Output::ok(fixture("ls.json")),
    );
    mock.on(
        &["image", "ls", "--format", "json"],
        Output::fail(1, fixture("stderr/service_down_ls.txt")),
    );
    mock.on(
        &["image", "ls", "--format", "json"],
        Output::ok(fixture("image_ls.json")),
    );
    mock.on(
        &["volume", "ls", "--format", "json"],
        Output::fail(1, fixture("stderr/service_down_ls.txt")),
    );
    mock.on(
        &["volume", "ls", "--format", "json"],
        Output::ok(fixture("volume_ls.json")),
    );
    mock.on(&["logs", "-n", "200", "qtest"], Output::ok(""));
    mock.on_stream(&["logs", "-f", "qtest"], vec![]);
    let mut h = Harness::started(mock);
    assert_eq!(h.state().screen, Screen::ServiceDown);

    // probe cadence: every 2nd tick while down
    h.engine.on_tick();
    h.pump();
    assert_eq!(
        h.state().screen,
        Screen::ServiceDown,
        "no probe on odd tick"
    );
    h.engine.on_tick();
    h.pump();

    assert_eq!(h.state().screen, Screen::Main);
    assert_eq!(h.state().containers.len(), 2);
});

engine_test!(stop_on_running_container_runs_without_confirmation, || {
    let mock = happy_mock();
    mock.on(&["stop", "qtest"], Output::ok("qtest\n"));
    let mut h = Harness::started(mock);

    h.engine.dispatch(Command::Run(UiAction::Stop));
    // pending is set synchronously, before the subprocess finishes
    assert_eq!(
        h.state().containers[0].pending.map(|p| p.kind),
        Some(ActionKind::Stop)
    );
    h.pump();

    assert!(
        h.mock
            .commands()
            .iter()
            .any(|c| c == "container stop qtest"),
        "{:?}",
        h.mock.commands()
    );
    // subprocess done → confirming phase until a poll shows "stopped"
    let p = h
        .state()
        .containers
        .iter()
        .find(|c| c.id == "qtest")
        .unwrap()
        .pending;
    assert!(
        matches!(p.map(|p| p.phase), Some(PendingPhase::Confirming(_))),
        "{p:?}"
    );
});

engine_test!(
    pending_clears_when_a_poll_confirms_the_expected_state,
    || {
        let mock = happy_mock();
        mock.on(&["stop", "qtest"], Output::ok("qtest\n"));
        // next poll shows qtest stopped
        let stopped = fixture_str("ls.json").replace(
            r#""startedDate":"2026-08-20T01:46:37Z","state":"running""#,
            r#""startedDate":"2026-08-20T01:46:37Z","state":"stopped""#,
        );
        let mut h = Harness::started(mock);
        h.engine.dispatch(Command::Run(UiAction::Stop));
        h.pump();

        h.mock
            .on(&["ls", "-a", "--format", "json"], Output::ok(stopped));
        h.engine.on_tick();
        h.pump();

        let qtest = h
            .state()
            .containers
            .iter()
            .find(|c| c.id == "qtest")
            .unwrap();
        assert_eq!(qtest.state, "stopped");
        assert!(qtest.pending.is_none(), "confirmed pending must clear");
        // the outcome is announced only now, at poll confirmation
        let toast = h.state().toast.clone().expect("confirmation toast");
        assert_eq!(toast.text, "stopped qtest");
    }
);

engine_test!(success_is_not_announced_before_a_poll_confirms_it, || {
    let mock = happy_mock();
    mock.on(&["stop", "qtest"], Output::ok("qtest\n"));
    let mut h = Harness::started(mock);
    h.engine.state.toast = None;

    h.engine.dispatch(Command::Run(UiAction::Stop));
    h.pump(); // subprocess exits ok, but polls still show "running"

    let toast = h.state().toast.clone();
    assert!(
        toast
            .as_ref()
            .is_none_or(|t| !t.text.starts_with("stopped")),
        "no success toast before poll confirmation: {toast:?}"
    );
});

engine_test!(pull_tag_defaulting_respects_registry_ports, || {
    let mock = happy_mock();
    mock.on_stream(
        &[
            "image",
            "pull",
            "localhost:5000/nginx:latest",
            "--progress",
            "plain",
        ],
        vec![StreamEvent::Exit(0)],
    );
    let mut h = Harness::started(mock);
    h.engine.dispatch(Command::SwitchPane(Pane::Images));
    h.pump();
    h.engine.dispatch(Command::Run(UiAction::Pull));
    for c in "localhost:5000/nginx".chars() {
        h.engine.dispatch(Command::OverlayChar(c));
    }
    h.engine.dispatch(Command::OverlaySubmit);
    h.pump();
    // the ':' in the registry port must not suppress the :latest default
    assert!(
        h.mock
            .commands()
            .iter()
            .any(|c| c == "container image pull localhost:5000/nginx:latest --progress plain"),
        "{:?}",
        h.mock.commands()
    );
});

engine_test!(images_parse_failures_do_not_count_toward_degraded, || {
    let mut h = Harness::started(happy_mock());
    h.mock
        .set(&["image", "ls", "--format", "json"], Output::ok("garbage"));
    // many slow-poll refreshes worth of failing image polls, healthy containers polls
    for _ in 0..30 {
        h.engine.on_tick();
    }
    h.pump();
    assert!(
        !h.state().degraded,
        "only the containers poll drives the degraded banner"
    );
});

engine_test!(unconfirmed_pending_clears_after_the_two_tick_cap, || {
    let mock = happy_mock();
    mock.on(&["stop", "qtest"], Output::ok("qtest\n"));
    let mut h = Harness::started(mock);
    h.engine.dispatch(Command::Run(UiAction::Stop));
    h.pump(); // ActionDone → Confirming(2), plus one immediate poll consuming a tick

    for _ in 0..2 {
        h.engine.on_tick();
        h.pump();
    }
    let qtest = h
        .state()
        .containers
        .iter()
        .find(|c| c.id == "qtest")
        .unwrap();
    assert!(
        qtest.pending.is_none(),
        "cap must clear stale pending: {:?}",
        qtest.pending
    );
});

engine_test!(deleted_container_confirms_by_disappearance, || {
    let mock = happy_mock();
    mock.on(&["delete", "old-batch"], Output::ok("old-batch\n"));
    let mut h = Harness::started(mock);
    h.engine.dispatch(Command::Move(1));
    h.pump();
    h.engine.dispatch(Command::Run(UiAction::Delete));
    h.engine.dispatch(Command::ConfirmYes);
    h.pump();

    // poll where old-batch is gone
    let ls: Vec<serde_json::Value> = serde_json::from_str(&fixture_str("ls.json")).unwrap();
    let only_qtest = serde_json::to_string(&vec![ls[0].clone()]).unwrap();
    h.mock
        .on(&["ls", "-a", "--format", "json"], Output::ok(only_qtest));
    h.engine.on_tick();
    h.pump();

    assert!(h.state().containers.iter().all(|c| c.id != "old-batch"));
    let toast = h.state().toast.clone().expect("toast");
    assert_eq!(toast.text, "deleted old-batch");
});

engine_test!(kill_requires_confirmation_showing_the_exact_command, || {
    let mock = happy_mock();
    mock.on(&["kill", "qtest"], Output::ok("qtest\n"));
    let mut h = Harness::started(mock);

    h.engine.dispatch(Command::Run(UiAction::Kill));
    match &h.state().overlay {
        Overlay::Confirm {
            command, action, ..
        } => {
            assert_eq!(command, "container kill qtest");
            assert_eq!(*action, ActionKind::Kill);
        }
        other => panic!("expected confirm overlay, got {other:?}"),
    }
    // nothing ran yet
    assert!(
        !h.mock
            .commands()
            .iter()
            .any(|c| c.starts_with("container kill"))
    );

    h.engine.dispatch(Command::ConfirmYes);
    h.pump();
    assert!(
        h.mock
            .commands()
            .iter()
            .any(|c| c == "container kill qtest")
    );
});

engine_test!(esc_cancels_a_confirmation_without_running_anything, || {
    let mut h = Harness::started(happy_mock());
    h.engine.dispatch(Command::Run(UiAction::Delete));
    assert!(matches!(h.state().overlay, Overlay::Confirm { .. }));

    h.engine.dispatch(Command::CloseOverlay);
    h.pump();
    assert_eq!(h.state().overlay, Overlay::None);
    assert!(
        !h.mock
            .commands()
            .iter()
            .any(|c| c.starts_with("container delete"))
    );
});

engine_test!(deleting_an_in_use_volume_is_blocked_with_an_error, || {
    let mut h = Harness::started(happy_mock());
    h.engine.dispatch(Command::SwitchPane(Pane::Volumes));
    h.pump();
    // qvol sorts first and is selected; it is in use by old-batch
    assert_eq!(h.state().selected[2].as_deref(), Some("qvol"));

    h.engine.dispatch(Command::Run(UiAction::Delete));
    h.pump();

    let toast = h.state().toast.clone().expect("toast");
    assert!(toast.error);
    assert!(toast.text.contains("in use"), "{}", toast.text);
    assert!(
        !h.mock
            .commands()
            .iter()
            .any(|c| c.contains("volume delete"))
    );
    // full detail lands in the message log
    assert!(h.state().messages.iter().any(|m| m.contains("old-batch")));
});

engine_test!(
    restart_is_one_pending_action_running_stop_then_start,
    || {
        let mock = happy_mock();
        mock.on(&["stop", "qtest"], Output::ok("qtest\n"));
        mock.on(&["start", "qtest"], Output::ok("qtest\n"));
        let mut h = Harness::started(mock);

        h.engine.dispatch(Command::Run(UiAction::Restart));
        assert_eq!(
            h.state().containers[0].pending.map(|p| p.kind),
            Some(ActionKind::Restart)
        );
        h.pump();

        let cmds = h.mock.commands();
        let stop = cmds
            .iter()
            .position(|c| c == "container stop qtest")
            .expect("stop ran");
        let start = cmds
            .iter()
            .position(|c| c == "container start qtest")
            .expect("start ran");
        assert!(stop < start, "stop must precede start: {cmds:?}");
    }
);

engine_test!(second_action_on_a_pending_entity_is_rejected, || {
    let mock = happy_mock();
    mock.on(&["stop", "qtest"], Output::ok("qtest\n"));
    let mut h = Harness::started(mock);

    h.engine.dispatch(Command::Run(UiAction::Stop));
    // without pumping, the first action is still pending
    h.engine.dispatch(Command::Run(UiAction::Stop));
    let toast = h.state().toast.clone().expect("toast");
    assert!(toast.text.contains("already pending"), "{}", toast.text);
});

engine_test!(external_stop_is_announced_but_bushel_stops_are_not, || {
    let mut h = Harness::started(happy_mock());
    let stopped = fixture_str("ls.json").replace(
        r#""startedDate":"2026-08-20T01:46:37Z","state":"running""#,
        r#""startedDate":"2026-08-20T01:46:37Z","state":"stopped""#,
    );
    h.mock
        .on(&["ls", "-a", "--format", "json"], Output::ok(stopped));
    h.engine.on_tick();
    h.pump();

    let toast = h.state().toast.clone().expect("external stop should toast");
    assert!(toast.text.contains("stopped externally"), "{}", toast.text);
    // and the diff is in the message log
    assert!(
        h.state()
            .messages
            .iter()
            .any(|m| m.contains("running → stopped"))
    );
});

engine_test!(action_failure_clears_pending_and_surfaces_stderr, || {
    let mock = happy_mock();
    mock.on(
        &["stop", "qtest"],
        Output::fail(1, "Error: some catastrophic thing"),
    );
    let mut h = Harness::started(mock);

    h.engine.dispatch(Command::Run(UiAction::Stop));
    h.pump();

    let qtest = h
        .state()
        .containers
        .iter()
        .find(|c| c.id == "qtest")
        .unwrap();
    assert!(
        qtest.pending.is_none(),
        "failure must clear pending immediately"
    );
    let toast = h.state().toast.clone().expect("toast");
    assert!(toast.error);
    assert!(
        h.state()
            .messages
            .iter()
            .any(|m| m.contains("some catastrophic thing"))
    );
});

engine_test!(not_found_during_action_is_a_notice_not_an_error, || {
    let mock = happy_mock();
    mock.on(
        &["stop", "qtest"],
        Output::fail(1, fixture("stderr/not_found_stop.txt")),
    );
    let mut h = Harness::started(mock);

    h.engine.dispatch(Command::Run(UiAction::Stop));
    h.pump();

    let toast = h.state().toast.clone().expect("toast");
    assert!(
        !toast.error,
        "NotFound is a status-bar notice only: {}",
        toast.text
    );
});

engine_test!(
    three_consecutive_parse_failures_degrade_but_keep_last_good_state,
    || {
        let mut h = Harness::started(happy_mock());
        assert_eq!(h.state().containers.len(), 2);

        h.mock
            .on(&["ls", "-a", "--format", "json"], Output::ok("garbage"));
        for i in 1..=3 {
            h.engine.on_tick();
            h.pump();
            let degraded = h.state().degraded;
            assert_eq!(degraded, i >= 3, "tick {i}: degraded={degraded}");
        }
        // last good state kept throughout
        assert_eq!(h.state().containers.len(), 2);

        // a good poll clears the banner
        h.mock.on(
            &["ls", "-a", "--format", "json"],
            Output::ok(fixture("ls.json")),
        );
        h.engine.on_tick();
        h.pump();
        assert!(!h.state().degraded);
    }
);

engine_test!(log_follower_backlog_then_follow_in_order, || {
    let mut h = Harness::started(happy_mock());
    // startup selected qtest (running) on Logs tab → follower live
    assert_eq!(h.engine.follower_id(), Some("qtest"));
    assert_eq!(
        h.state().log_lines,
        vec!["backlog-1", "backlog-2", "follow-1"]
    );
    assert!(!h.state().logs_loading);

    // switching to Inspect kills the follower
    h.engine.dispatch(Command::SetDetailTab(DetailTab::Inspect));
    h.pump();
    assert_eq!(h.engine.follower_id(), None);
});

engine_test!(
    follower_dies_when_a_poll_shows_the_container_stopped,
    || {
        let mut h = Harness::started(happy_mock());
        assert_eq!(h.engine.follower_id(), Some("qtest"));

        let stopped = fixture_str("ls.json").replace(
            r#""startedDate":"2026-08-20T01:46:37Z","state":"running""#,
            r#""startedDate":"2026-08-20T01:46:37Z","state":"stopped""#,
        );
        h.mock
            .on(&["ls", "-a", "--format", "json"], Output::ok(stopped));
        h.engine.on_tick();
        h.pump();

        assert_eq!(
            h.engine.follower_id(),
            None,
            "poll showed stopped → follower killed"
        );
    }
);

engine_test!(
    pull_streams_progress_and_refreshes_images_on_success,
    || {
        let mock = happy_mock();
        mock.on_stream(
            &["image", "pull", "alpine:latest", "--progress", "plain"],
            vec![
                StreamEvent::Stderr("pulling manifest".into()),
                StreamEvent::Stderr("done".into()),
                StreamEvent::Exit(0),
            ],
        );
        let mut h = Harness::started(mock);
        h.engine.dispatch(Command::SwitchPane(Pane::Images));
        h.pump();

        // pull prompt: tag defaults to latest when omitted
        h.engine.dispatch(Command::Run(UiAction::Pull));
        assert!(matches!(h.state().overlay, Overlay::PullInput { .. }));
        for c in "alpine".chars() {
            h.engine.dispatch(Command::OverlayChar(c));
        }
        h.engine.dispatch(Command::OverlaySubmit);
        h.pump();

        assert!(h.state().pull.is_none(), "pull finished");
        let toast = h.state().toast.clone().expect("toast");
        assert!(
            toast.text.contains("pulled alpine:latest"),
            "{}",
            toast.text
        );
        assert!(
            h.mock
                .commands()
                .iter()
                .any(|c| c == "container image pull alpine:latest --progress plain")
        );
    }
);

engine_test!(first_stats_sample_is_swallowed, || {
    let mut state = AppState::new(true);
    let ls: Vec<bushel::client::model::ContainerJson> =
        serde_json::from_slice(&fixture("ls.json")).unwrap();
    state.update_containers(&ls);

    let stats: Vec<bushel::client::model::StatsJson> = serde_json::from_str(
        r#"[{"id":"qtest","cpuUsageUsec":1000000,"memoryUsageBytes":100,"memoryLimitBytes":1000,"networkRxBytes":1000,"networkTxBytes":200,"blockReadBytes":300,"blockWriteBytes":400}]"#,
    )
    .unwrap();
    let prev = state.apply_stats(&stats, &HashMap::new(), Instant::now());

    let qtest = state.containers.iter().find(|c| c.id == "qtest").unwrap();
    assert!(qtest.cpu_percent.is_none(), "first sample has no CPU%");
    assert!(
        qtest.telemetry.is_empty(),
        "first sample is swallowed, no history"
    );
    assert!(
        prev.contains_key("qtest"),
        "baseline is kept for the next tick"
    );
});

engine_test!(
    second_stats_sample_first_differences_rates_into_the_ring,
    || {
        let mut state = AppState::new(true);
        let ls: Vec<bushel::client::model::ContainerJson> =
            serde_json::from_slice(&fixture("ls.json")).unwrap();
        state.update_containers(&ls);

        let now = Instant::now();
        let earlier = now - Duration::from_secs(1);
        let mut prev = HashMap::new();
        prev.insert(
            "qtest".to_string(),
            bushel::engine::StatsSnapshot {
                at: earlier,
                cpu_usage_usec: 1_000_000,
                network_rx_bytes: 1_000,
                network_tx_bytes: 200,
                block_read_bytes: 300,
                block_write_bytes: 400,
            },
        );

        // 1.5s of CPU over 1s wall → 150%; byte counters advance by known amounts.
        let stats: Vec<bushel::client::model::StatsJson> = serde_json::from_str(
            r#"[{"id":"qtest","cpuUsageUsec":2500000,"memoryUsageBytes":200,"memoryLimitBytes":1000,"networkRxBytes":3000,"networkTxBytes":700,"blockReadBytes":1300,"blockWriteBytes":400}]"#,
        )
        .unwrap();
        state.apply_stats(&stats, &prev, now);

        let qtest = state.containers.iter().find(|c| c.id == "qtest").unwrap();
        let cpu = qtest.cpu_percent.expect("cpu% set");
        assert!((cpu - 150.0).abs() < 5.0, "cpu% ≈ 150, got {cpu}");
        assert_eq!(qtest.mem_bytes, Some(200));
        assert_eq!(qtest.telemetry.len(), 1);
        let s = qtest.telemetry[0];
        assert!((s.cpu.unwrap() - 150.0).abs() < 5.0, "spark cpu ≈ 150");
        assert!((s.mem.unwrap() - 20.0).abs() < 0.01, "mem 200/1000 = 20%");
        assert_eq!(s.rx, Some(2_000));
        assert_eq!(s.tx, Some(500));
        assert_eq!(s.r, Some(1_000));
        assert_eq!(s.w, Some(0));
    }
);

fn stats_json(
    cpu: u64,
    mem: u64,
    limit: u64,
    rx: u64,
    tx: u64,
    r: u64,
    w: u64,
) -> Vec<bushel::client::model::StatsJson> {
    serde_json::from_str(&format!(
        r#"[{{"id":"qtest","cpuUsageUsec":{cpu},"memoryUsageBytes":{mem},"memoryLimitBytes":{limit},"networkRxBytes":{rx},"networkTxBytes":{tx},"blockReadBytes":{r},"blockWriteBytes":{w}}}]"#
    ))
    .unwrap()
}

fn seeded_containers() -> AppState {
    let mut state = AppState::new(true);
    let ls: Vec<bushel::client::model::ContainerJson> =
        serde_json::from_slice(&fixture("ls.json")).unwrap();
    state.update_containers(&ls);
    state
}

engine_test!(elapsed_zero_does_not_record_a_sample, || {
    let mut state = seeded_containers();
    let now = Instant::now();
    let prev = state.apply_stats(
        &stats_json(1_000, 100, 1000, 1000, 0, 0, 0),
        &HashMap::new(),
        now,
    );
    state.apply_stats(&stats_json(2_000, 100, 1000, 2000, 0, 0, 0), &prev, now);

    let qtest = state.containers.iter().find(|c| c.id == "qtest").unwrap();
    assert!(
        qtest.telemetry.is_empty(),
        "elapsed 0 must not first-difference: {:?}",
        qtest.telemetry
    );
    assert!(qtest.cpu_percent.is_none());
});

engine_test!(counter_reset_skips_that_rate_and_rebaselines, || {
    let mut state = seeded_containers();
    let t0 = Instant::now();
    let t1 = t0 + Duration::from_secs(1);
    let t2 = t1 + Duration::from_secs(1);

    let prev = state.apply_stats(
        &stats_json(1_000, 100, 1000, 5_000, 200, 300, 400),
        &HashMap::new(),
        t0,
    );
    let prev = state.apply_stats(&stats_json(2_000, 100, 1000, 100, 300, 400, 500), &prev, t1);

    let qtest = state.containers.iter().find(|c| c.id == "qtest").unwrap();
    let s = qtest.telemetry[0];
    assert_eq!(s.rx, None, "rx reset 5000→100 is not a rate");
    assert_eq!(s.tx, Some(100), "tx 200→300 over 1s");
    assert_eq!(s.r, Some(100));
    assert_eq!(s.w, Some(100));

    state.apply_stats(
        &stats_json(3_000, 100, 1000, 1_100, 400, 500, 600),
        &prev,
        t2,
    );
    let qtest = state.containers.iter().find(|c| c.id == "qtest").unwrap();
    assert_eq!(
        qtest.telemetry[0].rx,
        Some(1_000),
        "next tick diffs against the reset baseline"
    );
});

engine_test!(telemetry_ring_caps_at_five_minutes, || {
    let mut state = seeded_containers();
    let mut at = Instant::now();
    let mut prev = HashMap::new();
    // first sample swallowed, then 301 diffs → cap 300
    for i in 0..302u64 {
        let stats = stats_json(i * 1_000, 100, 1000, i * 10, 0, 0, 0);
        prev = state.apply_stats(&stats, &prev, at);
        at += Duration::from_secs(1);
    }
    let qtest = state.containers.iter().find(|c| c.id == "qtest").unwrap();
    assert_eq!(qtest.telemetry.len(), bushel::engine::TELEMETRY_HISTORY);
    // newest-first: the last diff used i=301 vs i=300 → rx 10 B/s
    assert_eq!(qtest.telemetry[0].rx, Some(10));
    // oldest kept is the sample from i=2 (first kept diff); i=1 was the first
    // diff and would have been evicted if we went one past the cap.
    assert_eq!(qtest.telemetry.back().unwrap().rx, Some(10));
});

engine_test!(telemetry_survives_a_containers_poll, || {
    let mut state = seeded_containers();
    let t0 = Instant::now();
    let prev = state.apply_stats(
        &stats_json(1_000, 100, 1000, 1_000, 0, 0, 0),
        &HashMap::new(),
        t0,
    );
    state.apply_stats(
        &stats_json(2_000, 100, 1000, 2_000, 0, 0, 0),
        &prev,
        t0 + Duration::from_secs(1),
    );
    assert_eq!(
        state
            .containers
            .iter()
            .find(|c| c.id == "qtest")
            .unwrap()
            .telemetry
            .len(),
        1
    );

    let ls: Vec<bushel::client::model::ContainerJson> =
        serde_json::from_slice(&fixture("ls.json")).unwrap();
    state.update_containers(&ls);
    let qtest = state.containers.iter().find(|c| c.id == "qtest").unwrap();
    assert_eq!(qtest.telemetry.len(), 1);
    assert_eq!(qtest.telemetry[0].rx, Some(1_000));
});

engine_test!(
    stats_derive_cpu_percent_from_consecutive_cumulative_samples,
    || {
        let mut state = AppState::new(true);
        let ls: Vec<bushel::client::model::ContainerJson> =
            serde_json::from_slice(&fixture("ls.json")).unwrap();
        state.update_containers(&ls);

        let now = Instant::now();
        let earlier = now - Duration::from_secs(1);
        let mut prev = HashMap::new();
        prev.insert(
            "qtest".to_string(),
            bushel::engine::StatsSnapshot {
                at: earlier,
                cpu_usage_usec: 1_000_000,
                network_rx_bytes: 0,
                network_tx_bytes: 0,
                block_read_bytes: 0,
                block_write_bytes: 0,
            },
        );

        // 1.5s of CPU over ~1s wall → ~150%
        let stats: Vec<bushel::client::model::StatsJson> = serde_json::from_str(
        r#"[{"id":"qtest","cpuUsageUsec":2500000,"memoryUsageBytes":4780032,"memoryLimitBytes":1073741824}]"#,
    )
    .unwrap();
        state.apply_stats(&stats, &prev, now);

        let qtest = state.containers.iter().find(|c| c.id == "qtest").unwrap();
        let cpu = qtest.cpu_percent.expect("cpu% set");
        assert!((cpu - 150.0).abs() < 5.0, "cpu% ≈ 150, got {cpu}");
        assert_eq!(qtest.mem_bytes, Some(4780032));
    }
);

engine_test!(fuzzy_filter_narrows_and_esc_clears, || {
    let mut h = Harness::started(happy_mock());
    h.engine.dispatch(Command::StartFilter);
    for c in "olbtc".chars() {
        h.engine.dispatch(Command::FilterChar(c)); // subsequence of "old-batch"
    }
    let visible = h.state().visible_rows();
    assert_eq!(visible.len(), 1);
    assert_eq!(h.state().containers[visible[0]].id, "old-batch");

    h.engine.dispatch(Command::Back); // esc clears filter
    assert!(h.state().filter.is_empty());
    assert_eq!(h.state().visible_rows().len(), 2);
});

engine_test!(action_menu_lists_valid_actions_for_the_selection, || {
    let mut h = Harness::started(happy_mock());
    // running container
    let keys: Vec<char> = h
        .state()
        .available_actions()
        .iter()
        .map(|a| a.key)
        .collect();
    assert_eq!(keys, vec!['s', 'r', 'K', 'd', 'P', 'e', 'l', 'i']);
    // stopped container
    h.engine.dispatch(Command::Move(1));
    h.pump();
    assert_eq!(h.state().selected_container().unwrap().id, "old-batch");
    let keys: Vec<char> = h
        .state()
        .available_actions()
        .iter()
        .map(|a| a.key)
        .collect();
    assert_eq!(keys, vec!['s', 'd', 'P', 'i']);
    // destructive tinting data
    assert!(
        h.state()
            .available_actions()
            .iter()
            .find(|a| a.key == 'd')
            .unwrap()
            .destructive
    );
});

engine_test!(inspect_is_fetched_lazily_and_cached, || {
    let mock = happy_mock();
    mock.on(&["inspect", "qtest"], Output::ok(fixture("inspect.json")));
    let mut h = Harness::started(mock);

    h.engine.dispatch(Command::SetDetailTab(DetailTab::Inspect));
    h.pump();
    assert!(h.state().inspect_cache.contains_key("qtest"));

    let calls_before = h.mock.calls().len();
    h.engine.dispatch(Command::SetDetailTab(DetailTab::Logs));
    h.pump();
    h.engine.dispatch(Command::SetDetailTab(DetailTab::Inspect));
    h.pump();
    // no refetch while cached
    let inspects = h.mock.calls()[calls_before..]
        .iter()
        .filter(|c| c.first().map(|s| s.as_str()) == Some("inspect"))
        .count();
    assert_eq!(inspects, 0);
});

engine_test!(exec_request_pauses_follower_and_resumes_after, || {
    let mut h = Harness::started(happy_mock());
    assert_eq!(h.engine.follower_id(), Some("qtest"));

    h.engine.dispatch(Command::Run(UiAction::Exec));
    assert_eq!(h.state().exec_request.as_deref(), Some("qtest"));

    let args = h.engine.prepare_exec();
    assert_eq!(args.join(" "), "exec -it qtest /bin/sh");
    assert_eq!(h.engine.follower_id(), None, "follower killed during exec");

    h.engine.after_exec();
    h.pump();
    assert_eq!(
        h.engine.follower_id(),
        Some("qtest"),
        "follower resynced after exec"
    );
});

engine_test!(prune_confirms_then_runs_as_a_bottom_bar_activity, || {
    let mock = happy_mock();
    mock.on(&["delete", "--all"], Output::ok("old-batch\n"));
    let mut h = Harness::started(mock);

    h.engine.dispatch(Command::Run(UiAction::Prune));
    match &h.state().overlay {
        Overlay::Confirm { command, .. } => assert_eq!(command, "container delete --all"),
        other => panic!("expected confirm, got {other:?}"),
    }
    h.engine.dispatch(Command::ConfirmYes);
    assert!(
        h.state().activity.is_some(),
        "prune shows as activity while running"
    );
    h.pump();
    assert!(h.state().activity.is_none());
    assert!(
        h.mock
            .commands()
            .iter()
            .any(|c| c == "container delete --all")
    );
});

engine_test!(selection_is_anchored_by_id_across_polls, || {
    let mut h = Harness::started(happy_mock());
    h.engine.dispatch(Command::Move(1));
    h.pump();
    assert_eq!(h.state().selected[0].as_deref(), Some("old-batch"));

    // a poll where qtest stops re-sorts the list; selection must stick to old-batch
    let stopped = fixture_str("ls.json").replace(
        r#""startedDate":"2026-08-20T01:46:37Z","state":"running""#,
        r#""startedDate":"2026-08-20T01:46:37Z","state":"stopped""#,
    );
    h.mock
        .on(&["ls", "-a", "--format", "json"], Output::ok(stopped));
    h.engine.on_tick();
    h.pump();

    assert_eq!(h.state().selected[0].as_deref(), Some("old-batch"));
});

engine_test!(first_run_splash_dwells_until_the_beat_ends, || {
    let mock = happy_mock();
    let mut h = {
        let mock = std::sync::Arc::new(mock);
        let client = Client::new(std::sync::Arc::clone(&mock));
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let mut engine = Engine::new(client, tx, false); // splash enabled
        engine.state.first_run = true;
        engine.start();
        let mut h = Harness { engine, rx, mock };
        h.pump();
        h
    };

    // data has landed, but the dwell hasn't elapsed → still on the splash
    assert!(h.state().first_data);
    assert_eq!(h.state().screen, Screen::Splash);

    // once the dwell has elapsed, the next check dissolves it
    h.engine.state.started_at = Instant::now() - Duration::from_secs(2);
    h.engine.maybe_dissolve_splash();
    assert_eq!(h.state().screen, Screen::Main);
});

engine_test!(non_first_run_splash_dissolves_the_moment_data_lands, || {
    let mock = happy_mock();
    let mut h = {
        let mock = std::sync::Arc::new(mock);
        let client = Client::new(std::sync::Arc::clone(&mock));
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let mut engine = Engine::new(client, tx, false); // splash enabled, not first run
        engine.start();
        let mut h = Harness { engine, rx, mock };
        h.pump();
        h
    };
    h.engine.maybe_dissolve_splash();
    assert_eq!(h.state().screen, Screen::Main);
});

engine_test!(any_key_skips_the_first_run_dwell, || {
    let mock = happy_mock();
    let mut h = {
        let mock = std::sync::Arc::new(mock);
        let client = Client::new(std::sync::Arc::clone(&mock));
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let mut engine = Engine::new(client, tx, false);
        engine.state.first_run = true;
        engine.start();
        let mut h = Harness { engine, rx, mock };
        h.pump();
        h
    };
    assert_eq!(h.state().screen, Screen::Splash);
    h.engine.dispatch(Command::SkipSplash);
    assert_eq!(h.state().screen, Screen::Main);
});

engine_test!(logs_start_wrapped_and_w_toggles_globally, || {
    let mut h = Harness::started(happy_mock());
    assert!(h.state().wrap, "a fresh process shows logs wrapped");

    h.engine.dispatch(Command::ToggleWrap);
    assert!(!h.state().wrap);

    // switching containers or panes does not change the mode
    h.engine.dispatch(Command::Move(1));
    h.pump();
    assert!(!h.state().wrap);
    h.engine.dispatch(Command::SwitchPane(Pane::Images));
    h.pump();
    assert!(!h.state().wrap);
    h.engine.dispatch(Command::SwitchPane(Pane::Containers));
    h.pump();
    assert!(!h.state().wrap);

    h.engine.dispatch(Command::ToggleWrap);
    assert!(h.state().wrap);
});

engine_test!(wrap_toggle_keeps_follow_and_the_paused_raw_line, || {
    let mut h = Harness::started(happy_mock());
    assert!(h.state().follow);
    h.engine.dispatch(Command::ToggleWrap);
    assert!(
        h.state().follow,
        "following stays on the tail across a wrap toggle"
    );

    h.engine.dispatch(Command::ToggleFollow);
    assert!(!h.state().follow);
    h.engine.dispatch(Command::SetDetailScroll(1));
    let scroll = h.state().detail_scroll;
    h.engine.dispatch(Command::ToggleWrap);
    assert!(!h.state().follow);
    assert_eq!(
        h.state().detail_scroll,
        scroll,
        "paused: the raw log line at the top is unchanged"
    );
});

engine_test!(keys_the_floor_sheet_omits_still_work_from_the_sheet, || {
    let mut h = Harness::started(happy_mock());
    h.engine.dispatch(Command::OpenActionMenu);
    // the floor sheet does not list l/i …
    assert!(
        !h.state()
            .menu_actions(true)
            .iter()
            .any(|i| i.key == 'l' || i.key == 'i')
    );
    // … but the key still switches the detail tab from inside the sheet
    h.engine.dispatch(Command::OverlayChar('i'));
    assert_eq!(h.state().detail_tab, DetailTab::Inspect);
    assert_eq!(h.state().overlay, Overlay::None);
});

engine_test!(help_scroll_resets_each_time_the_cheatsheet_opens, || {
    let mut h = Harness::started(happy_mock());
    h.engine.dispatch(Command::OpenHelp);
    h.engine.dispatch(Command::SetHelpScroll(7));
    assert_eq!(h.state().help_scroll, 7);
    h.engine.dispatch(Command::CloseOverlay);
    h.engine.dispatch(Command::OpenHelp);
    assert_eq!(h.state().help_scroll, 0);
});

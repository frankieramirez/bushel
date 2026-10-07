use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

pub const CONTAINER_BIN: &str = "container";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Output {
    pub fn ok(stdout: impl Into<Vec<u8>>) -> Self {
        Self {
            code: 0,
            stdout: stdout.into(),
            stderr: Vec::new(),
        }
    }

    pub fn fail(code: i32, stderr: impl Into<Vec<u8>>) -> Self {
        Self {
            code,
            stdout: Vec::new(),
            stderr: stderr.into(),
        }
    }

    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Stdout(String),
    Stderr(String),
    Exit(i32),
}

/// Output lines followed by exactly one terminal Exit event.
/// The drain deadline starts when the child exits or is killed. Readers get
/// up to two seconds to deliver remaining stdout and stderr, including queued
/// lines. A descendant retaining a pipe or a stalled receiver cannot hold the
/// stream open indefinitely: remaining readers are aborted and joined before
/// Exit, and any output still undelivered at the deadline is discarded.
pub type LineStream = mpsc::Receiver<StreamEvent>;

pub struct KillHandle {
    inner: Box<dyn FnOnce() + Send + Sync>,
}

impl KillHandle {
    pub fn new(f: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self { inner: Box::new(f) }
    }

    pub fn noop() -> Self {
        Self::new(|| {})
    }

    pub fn kill(self) {
        (self.inner)();
    }
}

impl std::fmt::Debug for KillHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KillHandle")
    }
}

#[async_trait]
pub trait Runner: Send + Sync + 'static {
    async fn run(&self, args: &[String]) -> std::io::Result<Output>;

    fn spawn_stream(&self, args: &[String]) -> std::io::Result<(LineStream, KillHandle)>;
}

#[derive(Debug, Default, Clone)]
pub struct CliRunner;

#[async_trait]
impl Runner for CliRunner {
    async fn run(&self, args: &[String]) -> std::io::Result<Output> {
        let out = Command::new(CONTAINER_BIN)
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await?;
        Ok(Output {
            code: out.status.code().unwrap_or(-1),
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }

    fn spawn_stream(&self, args: &[String]) -> std::io::Result<(LineStream, KillHandle)> {
        let mut command = Command::new(CONTAINER_BIN);
        command.args(args);
        spawn_command_stream(command)
    }
}

fn spawn_command_stream(mut command: Command) -> std::io::Result<(LineStream, KillHandle)> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let (tx, rx) = mpsc::channel(256);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut readers = tokio::task::JoinSet::new();

    if let Some(stdout) = stdout {
        let tx = tx.clone();
        readers.spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(StreamEvent::Stdout(line)).await.is_err() {
                    break;
                }
            }
        });
    }
    if let Some(stderr) = stderr {
        let tx = tx.clone();
        readers.spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(StreamEvent::Stderr(line)).await.is_err() {
                    break;
                }
            }
        });
    }

    let (kill_tx, mut kill_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        let code = tokio::select! {
            status = child.wait() => status.ok().and_then(|s| s.code()).unwrap_or(-1),
            _ = kill_rx.recv() => {
                let _ = child.kill().await;
                -1
            }
        };
        let drained = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while readers.join_next().await.is_some() {}
        })
        .await;
        if drained.is_err() {
            // A descendant may retain a pipe. Stop and join both readers so
            // no output can arrive after the terminal event.
            readers.abort_all();
            while readers.join_next().await.is_some() {}
        }
        let _ = tx.send(StreamEvent::Exit(code)).await;
    });

    Ok((
        rx,
        KillHandle::new(move || {
            let _ = kill_tx.try_send(());
        }),
    ))
}

#[derive(Default)]
pub struct MockRunner {
    responses: Mutex<HashMap<Vec<String>, Vec<Output>>>,
    last: Mutex<HashMap<Vec<String>, Output>>,
    default: Mutex<Option<Output>>,
    stream_lines: Mutex<HashMap<Vec<String>, Vec<StreamEvent>>>,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

impl MockRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on(&self, args: &[&str], out: Output) -> &Self {
        let key: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        self.responses
            .lock()
            .unwrap()
            .entry(key)
            .or_default()
            .push(out);
        self
    }

    pub fn set(&self, args: &[&str], out: Output) -> &Self {
        let key: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        self.responses.lock().unwrap().remove(&key);
        self.last.lock().unwrap().remove(&key);
        self.on(args, out)
    }

    pub fn on_default(&self, out: Output) -> &Self {
        *self.default.lock().unwrap() = Some(out);
        self
    }

    pub fn on_stream(&self, args: &[&str], events: Vec<StreamEvent>) -> &Self {
        let key: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        self.stream_lines.lock().unwrap().insert(key, events);
        self
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    pub fn commands(&self) -> Vec<String> {
        self.calls()
            .iter()
            .map(|c| format!("container {}", c.join(" ")))
            .collect()
    }
}

#[async_trait]
impl Runner for MockRunner {
    async fn run(&self, args: &[String]) -> std::io::Result<Output> {
        self.calls.lock().unwrap().push(args.to_vec());
        let mut responses = self.responses.lock().unwrap();
        if let Some(queue) = responses.get_mut(args) {
            if !queue.is_empty() {
                let out = queue.remove(0);
                self.last.lock().unwrap().insert(args.to_vec(), out.clone());
                return Ok(out);
            }
        }
        drop(responses);
        if let Some(out) = self.last.lock().unwrap().get(args).cloned() {
            return Ok(out);
        }
        if let Some(out) = self.default.lock().unwrap().clone() {
            return Ok(out);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("MockRunner: no response for `container {}`", args.join(" ")),
        ))
    }

    fn spawn_stream(&self, args: &[String]) -> std::io::Result<(LineStream, KillHandle)> {
        self.calls.lock().unwrap().push(args.to_vec());
        let events = self
            .stream_lines
            .lock()
            .unwrap()
            .get(args)
            .cloned()
            .unwrap_or_default();
        let (tx, rx) = mpsc::channel(events.len().max(1) + 1);
        tokio::spawn(async move {
            for e in events {
                if tx.send(e).await.is_err() {
                    return;
                }
            }
        });
        Ok((rx, KillHandle::noop()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn mock_replays_the_response_registered_for_the_exact_args() {
        let mock = MockRunner::new();
        mock.on(&["ls", "-a", "--format", "json"], Output::ok("[]"));

        let out = mock
            .run(&args(&["ls", "-a", "--format", "json"]))
            .await
            .unwrap();

        assert_eq!(out.code, 0);
        assert_eq!(out.stdout_str(), "[]");
    }

    #[tokio::test]
    async fn mock_pops_queued_responses_in_order_then_repeats_the_last() {
        let mock = MockRunner::new();
        mock.on(&["ls"], Output::ok("first"));
        mock.on(&["ls"], Output::ok("second"));

        assert_eq!(
            mock.run(&args(&["ls"])).await.unwrap().stdout_str(),
            "first"
        );
        assert_eq!(
            mock.run(&args(&["ls"])).await.unwrap().stdout_str(),
            "second"
        );
        assert_eq!(
            mock.run(&args(&["ls"])).await.unwrap().stdout_str(),
            "second"
        );
    }

    #[tokio::test]
    async fn mock_records_every_call_for_command_preview_assertions() {
        let mock = MockRunner::new();
        mock.on_default(Output::ok(""));

        mock.run(&args(&["stop", "web"])).await.unwrap();
        mock.run(&args(&["start", "web"])).await.unwrap();

        assert_eq!(
            mock.commands(),
            vec!["container stop web", "container start web"]
        );
    }

    #[tokio::test]
    async fn mock_stream_delivers_registered_events() {
        let mock = MockRunner::new();
        mock.on_stream(
            &["logs", "-f", "web"],
            vec![StreamEvent::Stdout("hello".into()), StreamEvent::Exit(0)],
        );

        let (mut rx, kill) = mock.spawn_stream(&args(&["logs", "-f", "web"])).unwrap();

        assert_eq!(rx.recv().await, Some(StreamEvent::Stdout("hello".into())));
        assert_eq!(rx.recv().await, Some(StreamEvent::Exit(0)));
        kill.kill();
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stream_exit_follows_all_final_stderr_lines() {
        for attempt in 0..200 {
            let mut command = Command::new("sh");
            command.args([
                "-c",
                "i=0; while [ $i -lt 300 ]; do echo line$i >&2; i=$((i+1)); done; exit 3",
            ]);
            let (mut stream, _kill) = spawn_command_stream(command).unwrap();
            let mut events = Vec::new();
            while let Some(event) = stream.recv().await {
                events.push(event);
            }
            assert_eq!(events.len(), 301, "attempt {attempt}");
            assert_eq!(
                events.last(),
                Some(&StreamEvent::Exit(3)),
                "attempt {attempt}"
            );
            for (i, event) in events[..300].iter().enumerate() {
                assert_eq!(
                    event,
                    &StreamEvent::Stderr(format!("line{i}")),
                    "attempt {attempt}"
                );
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stream_exit_follows_large_final_output_on_both_pipes() {
        let mut command = Command::new("sh");
        // Over two MiB total: larger than both pipe buffers and the 256-event
        // channel, exercising delivery under backpressure on both readers.
        command.args(["-c", r#"i=0; while [ $i -lt 4096 ]; do printf 'out%04d:%0256d\n' "$i" 0; printf 'err%04d:%0256d\n' "$i" 0 >&2; i=$((i+1)); done; exit 5"#]);
        let (mut stream, _kill) = spawn_command_stream(command).unwrap();
        let mut stdout = 0;
        let mut stderr = 0;
        let mut exited = false;
        while let Some(event) = stream.recv().await {
            assert!(!exited, "no event may follow Exit");
            match event {
                StreamEvent::Stdout(line) => {
                    assert_eq!(line, format!("out{stdout:04}:{:0256}", 0));
                    stdout += 1;
                }
                StreamEvent::Stderr(line) => {
                    assert_eq!(line, format!("err{stderr:04}:{:0256}", 0));
                    stderr += 1;
                }
                StreamEvent::Exit(code) => {
                    assert_eq!((stdout, stderr, code), (4096, 4096, 5));
                    exited = true;
                }
            }
            if (stdout + stderr) % 128 == 0 {
                tokio::task::yield_now().await;
            }
        }
        assert!(exited);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stream_exit_drains_stdout_stderr_and_unterminated_lines() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf 'out1\nout2'; printf 'err1\nerr2' >&2; exit 4"]);
        let (mut stream, _kill) = spawn_command_stream(command).unwrap();
        let mut events = Vec::new();
        while let Some(event) = stream.recv().await {
            events.push(event);
        }
        assert_eq!(events.pop(), Some(StreamEvent::Exit(4)));
        assert_eq!(
            events
                .iter()
                .filter_map(|e| match e {
                    StreamEvent::Stdout(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["out1", "out2"]
        );
        assert_eq!(
            events
                .iter()
                .filter_map(|e| match e {
                    StreamEvent::Stderr(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["err1", "err2"]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stream_exit_is_terminal_when_a_descendant_holds_a_pipe() {
        let mut command = Command::new("sh");
        // stdout closes promptly; stderr is retained by an isolated fixture.
        command.args([
            "-c",
            "(sleep 3; echo late >&2) >/dev/null & echo ready; exit 7",
        ]);
        let (mut stream, _kill) = spawn_command_stream(command).unwrap();
        let events = tokio::time::timeout(std::time::Duration::from_millis(2800), async {
            let mut events = Vec::new();
            while let Some(event) = stream.recv().await {
                events.push(event);
            }
            events
        })
        .await
        .expect("inherited pipes must not stall Exit indefinitely");
        assert_eq!(
            events,
            [StreamEvent::Stdout("ready".into()), StreamEvent::Exit(7)]
        );
    }

    #[tokio::test]
    async fn real_runner_reports_exit_code_and_stderr() {
        let out = CliRunner.run(&args(&["--version"])).await;
        let Ok(out) = out else { return };
        assert_eq!(out.code, 0);
        assert!(
            out.stdout_str().contains("container CLI version"),
            "{}",
            out.stdout_str()
        );
    }
}

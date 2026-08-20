//! The low-level subprocess seam. Every `container` CLI invocation passes through
//! a [`Runner`]; the real one spawns processes, the mock one replays fixtures.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// The binary bushel wraps. Never linked as a framework — always a subprocess.
pub const CONTAINER_BIN: &str = "container";

/// A finished invocation. `code` is the process exit status (128 + signal if killed).
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

/// One line (or the terminal exit code) from a long-lived subprocess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Stdout(String),
    Stderr(String),
    Exit(i32),
}

/// Receiver end of a streaming invocation.
pub type LineStream = mpsc::Receiver<StreamEvent>;

/// Kills the subprocess behind a [`LineStream`]. `logs -f` never exits on its own
/// when a container stops, so bushel always holds one of these.
pub struct KillHandle {
    inner: Box<dyn FnOnce() + Send + Sync>,
}

impl KillHandle {
    pub fn new(f: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self { inner: Box::new(f) }
    }

    /// A handle over a process that has already gone; killing it is a no-op.
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
    /// Run to completion, collecting stdout and stderr.
    async fn run(&self, args: &[String]) -> std::io::Result<Output>;

    /// Spawn a long-lived process, streaming its output line by line.
    fn spawn_stream(&self, args: &[String]) -> std::io::Result<(LineStream, KillHandle)>;
}

/// The real runner: `container <args>` via tokio, no PTY.
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
        let mut child = Command::new(CONTAINER_BIN)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let (tx, rx) = mpsc::channel(256);
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        if let Some(stdout) = stdout {
            let tx = tx.clone();
            tokio::spawn(async move {
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
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx.send(StreamEvent::Stderr(line)).await.is_err() {
                        break;
                    }
                }
            });
        }

        // The child is owned by the waiter task; killing goes through this channel so
        // the handle stays `Send + Sync` without a mutex around the child.
        let (kill_tx, mut kill_rx) = mpsc::channel::<()>(1);
        tokio::spawn(async move {
            let code = tokio::select! {
                status = child.wait() => status.ok().and_then(|s| s.code()).unwrap_or(-1),
                _ = kill_rx.recv() => {
                    let _ = child.kill().await;
                    -1
                }
            };
            let _ = tx.send(StreamEvent::Exit(code)).await;
        });

        Ok((
            rx,
            KillHandle::new(move || {
                let _ = kill_tx.try_send(());
            }),
        ))
    }
}

/// Replays canned outputs keyed on the exact argument vector. Used by the Client
/// fixture tests and the end-to-end engine tests.
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

    /// Queue one response for `args`. Each call pops the oldest queued response;
    /// once the queue drains, the most recently returned one replays.
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

    /// Replace everything registered for `args` with a single response.
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

    /// Every argument vector seen so far, in order.
    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    /// The calls rendered the way the command preview renders them.
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

    #[tokio::test]
    async fn real_runner_reports_exit_code_and_stderr() {
        // `container --version` is client-side and works with the service down.
        let out = CliRunner.run(&args(&["--version"])).await;
        let Ok(out) = out else { return }; // CLI absent (CI): nothing to assert.
        assert_eq!(out.code, 0);
        assert!(
            out.stdout_str().contains("container CLI version"),
            "{}",
            out.stdout_str()
        );
    }
}

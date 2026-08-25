//! The typed layer above the Runner: builds argument vectors, parses JSON into
//! entities, classifies errors. All version fragility is confined here and
//! covered by per-version fixtures in `fixtures/`.

pub mod error;
pub mod model;
pub mod version;

use std::sync::Arc;
use std::time::Duration;

pub use error::CliError;
pub use model::*;

use crate::runner::{KillHandle, LineStream, Output, Runner};

/// Deadline on reads. Mutating actions get none (pull and prune can run minutes).
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Log backlog requested before following.
pub const LOG_BACKLOG_LINES: u32 = 200;

pub type Result<T> = std::result::Result<T, CliError>;

pub struct Client<R: Runner> {
    runner: Arc<R>,
}

// derive(Clone) would require R: Clone; the Arc clones regardless.
impl<R: Runner> Clone for Client<R> {
    fn clone(&self) -> Self {
        Self {
            runner: Arc::clone(&self.runner),
        }
    }
}

fn to_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

/// Render an argument vector the way the command preview shows it.
pub fn preview(args: &[&str]) -> String {
    format!("container {}", args.join(" "))
}

impl<R: Runner> Client<R> {
    pub fn new(runner: Arc<R>) -> Self {
        Self { runner }
    }

    pub fn runner(&self) -> &Arc<R> {
        &self.runner
    }

    /// Run a read (10s deadline), expect exit 0, parse stdout as JSON.
    async fn read_json<T: serde::de::DeserializeOwned>(&self, args: &[&str]) -> Result<T> {
        let out = self.read(args).await?;
        serde_json::from_slice(&out.stdout).map_err(|e| CliError::ParseFailure {
            raw: format!("{}: {e}\nstdout: {}", preview(args), out.stdout_str()),
        })
    }

    async fn read(&self, args: &[&str]) -> Result<Output> {
        let argv = to_args(args);
        let fut = self.runner.run(&argv);
        let out = tokio::time::timeout(READ_TIMEOUT, fut)
            .await
            .map_err(|_| CliError::Timeout)?
            .map_err(|e| CliError::Other {
                raw: format!("{}: {e}", preview(args)),
            })?;
        if out.code != 0 {
            return Err(CliError::classify(out.code, &out.stderr_str()));
        }
        Ok(out)
    }

    /// Run a mutation. No deadline; non-zero exit classifies.
    async fn mutate(&self, args: &[&str]) -> Result<Output> {
        let out = self
            .runner
            .run(&to_args(args))
            .await
            .map_err(|e| CliError::Other {
                raw: format!("{}: {e}", preview(args)),
            })?;
        if out.code != 0 {
            return Err(CliError::classify(out.code, &out.stderr_str()));
        }
        Ok(out)
    }

    // ---- reads --------------------------------------------------------------

    pub async fn list_containers(&self) -> Result<Vec<ContainerJson>> {
        self.read_json(&["ls", "-a", "--format", "json"]).await
    }

    pub async fn list_images(&self) -> Result<Vec<ImageJson>> {
        self.read_json(&["image", "ls", "--format", "json"]).await
    }

    pub async fn list_volumes(&self) -> Result<Vec<VolumeJson>> {
        self.read_json(&["volume", "ls", "--format", "json"]).await
    }

    pub async fn stats(&self) -> Result<Vec<StatsJson>> {
        self.read_json(&["stats", "--no-stream", "--format", "json"])
            .await
    }

    /// Health probe. Exit 1 with parseable "not running" output is `ServiceDown`.
    pub async fn system_status(&self) -> Result<SystemStatusJson> {
        let args = ["system", "status", "--format", "json"];
        let argv = to_args(&args);
        let fut = self.runner.run(&argv);
        let out = tokio::time::timeout(READ_TIMEOUT, fut)
            .await
            .map_err(|_| CliError::Timeout)?
            .map_err(|e| CliError::Other {
                raw: format!("{}: {e}", preview(&args)),
            })?;
        // Down: exit 1, but stdout may still carry valid JSON with a non-running
        // status, or the plain-text "apiserver is not running…" line.
        if out.code != 0 {
            if let Ok(s) = serde_json::from_slice::<SystemStatusJson>(&out.stdout) {
                if !s.is_running() {
                    return Err(CliError::ServiceDown {
                        raw: format!("service status: {}", s.status),
                    });
                }
            }
            let combined = format!("{}{}", out.stdout_str(), out.stderr_str());
            return Err(CliError::ServiceDown {
                raw: combined.trim().to_string(),
            });
        }
        serde_json::from_slice(&out.stdout).map_err(|e| CliError::ParseFailure {
            raw: format!("{}: {e}\nstdout: {}", preview(&args), out.stdout_str()),
        })
    }

    /// Raw pretty-printed inspect JSON, exactly as the CLI emits it.
    pub async fn inspect_container(&self, id: &str) -> Result<String> {
        Ok(self.read(&["inspect", id]).await?.stdout_str())
    }

    pub async fn inspect_image(&self, reference: &str) -> Result<String> {
        Ok(self
            .read(&["image", "inspect", reference])
            .await?
            .stdout_str())
    }

    pub async fn inspect_volume(&self, name: &str) -> Result<String> {
        Ok(self.read(&["volume", "inspect", name]).await?.stdout_str())
    }

    /// Bounded log tail (`logs -n 200`), split into lines.
    pub async fn logs_backlog(&self, id: &str) -> Result<Vec<String>> {
        let args = Self::logs_backlog_args(id);
        let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let out = self.read(&borrowed).await?;
        Ok(out.stdout_str().lines().map(str::to_string).collect())
    }

    pub async fn version(&self) -> Result<String> {
        Ok(self.read(&["--version"]).await?.stdout_str())
    }

    // ---- mutations (arg vectors are the single source for command previews) --

    pub fn start_args(id: &str) -> Vec<String> {
        to_args(&["start", id])
    }

    pub fn stop_args(id: &str) -> Vec<String> {
        to_args(&["stop", id])
    }

    pub fn kill_args(id: &str) -> Vec<String> {
        to_args(&["kill", id])
    }

    pub fn delete_container_args(id: &str) -> Vec<String> {
        to_args(&["delete", id])
    }

    pub fn prune_containers_args() -> Vec<String> {
        // Without --force the CLI refuses running containers, so --all is exactly
        // "delete all stopped" — the prune bushel wants.
        to_args(&["delete", "--all"])
    }

    pub fn delete_image_args(reference: &str) -> Vec<String> {
        to_args(&["image", "delete", reference])
    }

    pub fn prune_images_args() -> Vec<String> {
        to_args(&["image", "prune"])
    }

    pub fn pull_args(reference: &str) -> Vec<String> {
        to_args(&["image", "pull", reference, "--progress", "plain"])
    }

    pub fn delete_volume_args(name: &str) -> Vec<String> {
        to_args(&["volume", "delete", name])
    }

    pub fn prune_volumes_args() -> Vec<String> {
        to_args(&["volume", "prune"])
    }

    pub fn system_start_args() -> Vec<String> {
        // bare `system start` blocks on an interactive kernel prompt
        to_args(&["system", "start", "--enable-kernel-install"])
    }

    pub fn exec_shell_args(id: &str) -> Vec<String> {
        to_args(&["exec", "-it", id, "/bin/sh"])
    }

    pub fn logs_backlog_args(id: &str) -> Vec<String> {
        vec![
            "logs".into(),
            "-n".into(),
            LOG_BACKLOG_LINES.to_string(),
            id.into(),
        ]
    }

    pub fn logs_follow_args(id: &str) -> Vec<String> {
        to_args(&["logs", "-f", id])
    }

    /// Run a pre-built mutation argument vector (what a confirmed preview executes).
    pub async fn run_action(&self, args: &[String]) -> Result<Output> {
        let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.mutate(&borrowed).await
    }

    /// Spawn the follow subprocess (`logs -f`). Caller owns the KillHandle — the process
    /// never exits on its own when the container stops.
    pub fn spawn_follow(&self, id: &str) -> std::io::Result<(LineStream, KillHandle)> {
        self.runner.spawn_stream(&Self::logs_follow_args(id))
    }

    /// Spawn a pull; its plain-progress stderr lines feed the detail pane.
    pub fn spawn_pull(&self, reference: &str) -> std::io::Result<(LineStream, KillHandle)> {
        self.runner.spawn_stream(&Self::pull_args(reference))
    }

    /// Spawn the service start; its output lines feed the takeover view.
    pub fn spawn_system_start(&self) -> std::io::Result<(LineStream, KillHandle)> {
        self.runner.spawn_stream(&Self::system_start_args())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::MockRunner;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!("fixtures/1.2.0/{name}")).unwrap()
    }

    fn client_with(mock: MockRunner) -> Client<MockRunner> {
        Client::new(Arc::new(mock))
    }

    #[tokio::test]
    async fn list_containers_parses_the_captured_ls_fixture() {
        let mock = MockRunner::new();
        mock.on(
            &["ls", "-a", "--format", "json"],
            Output::ok(fixture("ls.json")),
        );
        let client = client_with(mock);

        let containers = client.list_containers().await.unwrap();

        assert_eq!(containers.len(), 2);
        let running = &containers[0];
        assert_eq!(running.id, "qtest");
        assert!(running.is_running());
        assert_eq!(running.image_reference(), "docker.io/library/alpine:latest");
        let stopped = &containers[1];
        assert!(!stopped.is_running());
        assert_eq!(stopped.volume_sources().collect::<Vec<_>>(), vec!["qvol"]);
    }

    #[tokio::test]
    async fn list_images_parses_fixture_and_filters_attestation_variants_for_size() {
        let mock = MockRunner::new();
        mock.on(
            &["image", "ls", "--format", "json"],
            Output::ok(fixture("image_ls.json")),
        );
        let client = client_with(mock);

        let images = client.list_images().await.unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].reference(), "docker.io/library/alpine:latest");
        // arm64 variant (3623807), not the os=unknown attestation (561)
        assert_eq!(images[0].display_size(), Some(3623807));
    }

    #[tokio::test]
    async fn list_volumes_parses_fixture() {
        let mock = MockRunner::new();
        mock.on(
            &["volume", "ls", "--format", "json"],
            Output::ok(fixture("volume_ls.json")),
        );
        let client = client_with(mock);

        let volumes = client.list_volumes().await.unwrap();

        assert_eq!(
            volumes.iter().map(|v| v.name()).collect::<Vec<_>>(),
            vec!["qvol", "scratch"]
        );
    }

    #[tokio::test]
    async fn stats_parses_cumulative_cpu_counter() {
        let mock = MockRunner::new();
        mock.on(
            &["stats", "--no-stream", "--format", "json"],
            Output::ok(fixture("stats.json")),
        );
        let client = client_with(mock);

        let stats = client.stats().await.unwrap();

        assert_eq!(stats[0].id, "qtest");
        assert_eq!(stats[0].cpu_usage_usec, 35372);
        assert_eq!(stats[0].memory_usage_bytes, 4780032);
    }

    #[tokio::test]
    async fn system_status_running_fixture_parses() {
        let mock = MockRunner::new();
        mock.on(
            &["system", "status", "--format", "json"],
            Output::ok(fixture("system_status_running.json")),
        );
        let client = client_with(mock);

        assert!(client.system_status().await.unwrap().is_running());
    }

    #[tokio::test]
    async fn system_status_down_exits_1_with_json_and_maps_to_service_down() {
        // Captured live: exit 1, valid JSON with status "unregistered" on stdout.
        let mock = MockRunner::new();
        mock.on(
            &["system", "status", "--format", "json"],
            Output {
                code: 1,
                stdout: fixture("system_status_down.json"),
                stderr: Vec::new(),
            },
        );
        let client = client_with(mock);

        let err = client.system_status().await.unwrap_err();
        assert!(matches!(err, CliError::ServiceDown { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn read_failure_with_xpc_stderr_classifies_as_service_down() {
        let mock = MockRunner::new();
        mock.on(
            &["ls", "-a", "--format", "json"],
            Output::fail(1, fixture("stderr/service_down_ls.txt")),
        );
        let client = client_with(mock);

        let err = client.list_containers().await.unwrap_err();
        assert!(matches!(err, CliError::ServiceDown { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn garbage_stdout_is_a_parse_failure_carrying_the_offending_output() {
        let mock = MockRunner::new();
        mock.on(&["ls", "-a", "--format", "json"], Output::ok("not json"));
        let client = client_with(mock);

        let err = client.list_containers().await.unwrap_err();
        match err {
            CliError::ParseFailure { raw } => assert!(raw.contains("not json"), "{raw}"),
            other => panic!("expected ParseFailure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn inspect_returns_raw_pretty_json_untouched() {
        let mock = MockRunner::new();
        mock.on(&["inspect", "qtest"], Output::ok(fixture("inspect.json")));
        let client = client_with(mock);

        let text = client.inspect_container("qtest").await.unwrap();
        assert!(text.trim_start().starts_with('['));
        assert!(text.contains("\"qtest\""));
    }

    #[test]
    fn action_arg_vectors_render_the_documented_command_previews() {
        assert_eq!(preview(&["stop", "web"]), "container stop web");
        assert_eq!(Client::<MockRunner>::kill_args("web").join(" "), "kill web");
        assert_eq!(
            Client::<MockRunner>::prune_containers_args().join(" "),
            "delete --all"
        );
        assert_eq!(
            Client::<MockRunner>::prune_images_args().join(" "),
            "image prune"
        );
        assert_eq!(
            Client::<MockRunner>::prune_volumes_args().join(" "),
            "volume prune"
        );
        assert_eq!(
            Client::<MockRunner>::system_start_args().join(" "),
            "system start --enable-kernel-install"
        );
        assert_eq!(
            Client::<MockRunner>::pull_args("alpine:latest").join(" "),
            "image pull alpine:latest --progress plain"
        );
        assert_eq!(
            Client::<MockRunner>::exec_shell_args("web").join(" "),
            "exec -it web /bin/sh"
        );
        assert_eq!(
            Client::<MockRunner>::logs_backlog_args("web").join(" "),
            "logs -n 200 web"
        );
        assert_eq!(
            Client::<MockRunner>::logs_follow_args("web").join(" "),
            "logs -f web"
        );
    }
}

#[cfg(test)]
mod real_capture_tests {
    //! Verbatim captures from `container` 1.2.0 on this machine (2026-08-20).
    //! These only assert "the real shapes parse" — scenario tests use the
    //! curated fixtures above.

    use super::*;

    fn real(name: &str) -> Vec<u8> {
        std::fs::read(format!("fixtures/1.2.0/real/{name}")).unwrap()
    }

    #[test]
    fn real_ls_parses() {
        let list: Vec<ContainerJson> = serde_json::from_slice(&real("ls.json")).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "bushel-smoke");
        assert!(list[0].is_running());
        assert_eq!(list[0].image_reference(), "docker.io/library/alpine:latest");
    }

    #[test]
    fn real_ls_with_mounts_parses() {
        // Captured 2026-08-24: mount `type` is a tagged object ({"virtiofs":{}},
        // {"volume":{"name":…}}), not a string, and a volume's name lives in the
        // tag body — the source is the host path to its volume.img.
        let list: Vec<ContainerJson> =
            serde_json::from_slice(&real("ls_mounts.json")).unwrap();
        let fixture = list.iter().find(|c| c.id == "bushel-fixture").unwrap();
        assert_eq!(
            fixture.volume_sources().collect::<Vec<_>>(),
            vec!["bushel-fixture-vol"]
        );
        // bind (virtiofs) mounts parse but contribute no volume sources
        let comicarr = list.iter().find(|c| c.id == "comicarr").unwrap();
        assert_eq!(comicarr.volume_sources().count(), 0);
    }

    #[test]
    fn real_image_ls_parses() {
        let list: Vec<ImageJson> = serde_json::from_slice(&real("image_ls.json")).unwrap();
        assert!(list.iter().any(|i| i.reference().contains("alpine")));
        assert!(list[0].display_size().is_some());
    }

    #[test]
    fn real_volume_ls_parses() {
        let list: Vec<VolumeJson> = serde_json::from_slice(&real("volume_ls.json")).unwrap();
        assert_eq!(list[0].name(), "bushel-smoke-vol");
    }

    #[test]
    fn real_stats_parses() {
        let list: Vec<StatsJson> = serde_json::from_slice(&real("stats.json")).unwrap();
        assert_eq!(list[0].id, "bushel-smoke");
        assert!(list[0].memory_usage_bytes > 0);
    }

    #[test]
    fn real_system_status_parses() {
        let s: SystemStatusJson = serde_json::from_slice(&real("system_status.json")).unwrap();
        assert!(s.is_running());
    }

    #[test]
    fn real_inspect_is_a_pretty_printed_array() {
        let text = String::from_utf8(real("inspect.json")).unwrap();
        let v: Vec<ContainerJson> = serde_json::from_str(&text).unwrap();
        assert_eq!(v[0].id, "bushel-smoke");
    }
}

pub mod error;
pub mod model;
pub mod version;

use std::sync::Arc;
use std::time::Duration;

pub use error::CliError;
pub use model::*;

use crate::runner::{KillHandle, LineStream, Output, Runner};

pub const READ_TIMEOUT: Duration = Duration::from_secs(10);

pub const LOG_BACKLOG_LINES: u32 = 200;

pub type Result<T> = std::result::Result<T, CliError>;

pub struct Client<R: Runner> {
    runner: Arc<R>,
}

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

    pub async fn list_containers(&self) -> Result<Vec<ContainerJson>> {
        self.read_json(&["ls", "-a", "--format", "json"]).await
    }

    pub async fn list_images(&self) -> Result<Vec<ImageJson>> {
        self.read_json(&["image", "ls", "--format", "json"]).await
    }

    pub async fn list_volumes(&self) -> Result<Vec<VolumeJson>> {
        self.read_json(&["volume", "ls", "--format", "json"]).await
    }

    pub async fn list_networks(&self) -> Result<Vec<NetworkJson>> {
        self.read_json(&["network", "list", "--format", "json"])
            .await
    }

    pub async fn stats(&self) -> Result<Vec<StatsJson>> {
        self.read_json(&["stats", "--no-stream", "--format", "json"])
            .await
    }

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
        if out.code != 0 {
            if let Ok(s) = serde_json::from_slice::<SystemStatusJson>(&out.stdout) {
                if !s.is_running() {
                    return Err(CliError::ServiceDown {
                        raw: format!("service status: {}", s.status),
                    });
                }
            }
            let stdout = out.stdout_str();
            let stderr = out.stderr_str();
            let combined = [stdout.trim(), stderr.trim()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            return Err(CliError::classify(out.code, &combined));
        }
        serde_json::from_slice(&out.stdout).map_err(|e| CliError::ParseFailure {
            raw: format!("{}: {e}\nstdout: {}", preview(&args), out.stdout_str()),
        })
    }

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

    pub async fn inspect_network(&self, name: &str) -> Result<String> {
        Ok(self.read(&["network", "inspect", name]).await?.stdout_str())
    }

    pub async fn logs_backlog(&self, id: &str) -> Result<Vec<String>> {
        let args = Self::logs_backlog_args(id);
        let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let out = self.read(&borrowed).await?;
        Ok(out.stdout_str().lines().map(str::to_string).collect())
    }

    pub async fn version(&self) -> Result<String> {
        Ok(self.read(&["--version"]).await?.stdout_str())
    }

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

    pub fn tag_image_args(source: &str, target: &str) -> Vec<String> {
        to_args(&["image", "tag", source, target])
    }

    pub fn delete_volume_args(name: &str) -> Vec<String> {
        to_args(&["volume", "delete", name])
    }

    pub fn create_volume_args(name: &str) -> Vec<String> {
        to_args(&["volume", "create", name])
    }

    pub async fn volume_create(&self, name: &str) -> Result<Output> {
        let args = Self::create_volume_args(name);
        let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.mutate(&borrowed).await
    }

    pub fn prune_volumes_args() -> Vec<String> {
        to_args(&["volume", "prune"])
    }

    pub fn system_start_args() -> Vec<String> {
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

    pub async fn run_action(&self, args: &[String]) -> Result<Output> {
        let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.mutate(&borrowed).await
    }

    pub fn spawn_follow(&self, id: &str) -> std::io::Result<(LineStream, KillHandle)> {
        self.runner.spawn_stream(&Self::logs_follow_args(id))
    }

    pub fn spawn_pull(&self, reference: &str) -> std::io::Result<(LineStream, KillHandle)> {
        self.runner.spawn_stream(&Self::pull_args(reference))
    }

    pub fn spawn_system_start(&self) -> std::io::Result<(LineStream, KillHandle)> {
        self.runner.spawn_stream(&Self::system_start_args())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::MockRunner;

    const FIXTURE_VERSIONS: [&str; 2] = ["1.2.0", "1.3.1"];

    fn fixture_version(version: &str, name: &str) -> Vec<u8> {
        std::fs::read(format!("fixtures/{version}/{name}")).unwrap()
    }

    fn client_with(mock: MockRunner) -> Client<MockRunner> {
        Client::new(Arc::new(mock))
    }

    #[tokio::test]
    async fn list_containers_parses_the_captured_ls_fixture() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["ls", "-a", "--format", "json"],
                Output::ok(fixture_version(version, "ls.json")),
            );
            let client = client_with(mock);

            let containers = client.list_containers().await.unwrap();

            assert_eq!(containers.len(), 2, "fixture generation {version}");
            let running = &containers[0];
            assert_eq!(running.id, "qtest");
            assert!(running.is_running());
            assert_eq!(running.image_reference(), "docker.io/library/alpine:latest");
            let stopped = &containers[1];
            assert!(!stopped.is_running());
            assert_eq!(stopped.volume_sources().collect::<Vec<_>>(), vec!["qvol"]);
            assert_eq!(
                running.network_attachments().collect::<Vec<_>>(),
                vec![("default", Some("192.168.64.2/24"))]
            );
            assert_eq!(stopped.network_attachments().count(), 0);
        }
    }

    #[tokio::test]
    async fn list_images_parses_fixture_and_filters_attestation_variants_for_size() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["image", "ls", "--format", "json"],
                Output::ok(fixture_version(version, "image_ls.json")),
            );
            let client = client_with(mock);

            let images = client.list_images().await.unwrap();

            assert_eq!(images.len(), 2, "fixture generation {version}");
            assert_eq!(images[0].reference(), "docker.io/library/alpine:latest");
            assert_eq!(images[0].display_size(), Some(3623807));
        }
    }

    #[tokio::test]
    async fn volume_create_runs_name_only_create() {
        let mock = MockRunner::new();
        mock.on(&["volume", "create", "scratch2"], Output::ok("scratch2\n"));
        let client = client_with(mock);

        let out = client.volume_create("scratch2").await.unwrap();

        assert_eq!(out.code, 0);
        assert_eq!(out.stdout_str(), "scratch2\n");
    }

    #[tokio::test]
    async fn list_volumes_parses_fixture() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["volume", "ls", "--format", "json"],
                Output::ok(fixture_version(version, "volume_ls.json")),
            );
            let client = client_with(mock);

            let volumes = client.list_volumes().await.unwrap();

            assert_eq!(
                volumes.iter().map(|v| v.name()).collect::<Vec<_>>(),
                vec!["qvol", "scratch"],
                "fixture generation {version}"
            );
        }
    }

    #[tokio::test]
    async fn list_networks_parses_fixture() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["network", "list", "--format", "json"],
                Output::ok(fixture_version(version, "network_ls.json")),
            );
            let client = client_with(mock);

            let networks = client.list_networks().await.unwrap();

            assert_eq!(
                networks.iter().map(|n| n.name()).collect::<Vec<_>>(),
                vec!["default", "foo", "internal"],
                "fixture generation {version}"
            );
            assert!(networks[0].is_builtin());
            assert_eq!(networks[0].mode(), "nat");
            assert_eq!(networks[0].ipv4_subnet(), Some("192.168.64.0/24"));
            assert!(!networks[1].is_builtin());
            assert_eq!(networks[2].mode(), "hostOnly");
            assert_eq!(networks[2].ipv4_subnet(), Some("192.168.66.0/24"));
        }
    }

    #[test]
    fn network_json_tolerates_extra_keys_and_legacy_subnet() {
        let raw = r#"[{
            "id": "legacy",
            "unexpected": true,
            "configuration": {
                "name": "legacy",
                "mode": "nat",
                "subnet": "10.0.0.0/24",
                "pluginInfo": {"plugin": "old"},
                "extra": 1
            },
            "status": {
                "ipv4Gateway": "10.0.0.1",
                "mystery": "ignored"
            }
        }]"#;
        let networks: Vec<NetworkJson> = serde_json::from_str(raw).unwrap();
        assert_eq!(networks[0].name(), "legacy");
        assert_eq!(networks[0].ipv4_subnet(), Some("10.0.0.0/24"));
        assert!(!networks[0].is_builtin());
    }

    #[tokio::test]
    async fn inspect_network_returns_raw_pretty_json() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["network", "inspect", "default"],
                Output::ok(fixture_version(version, "network_inspect.json")),
            );
            let client = client_with(mock);

            let text = client.inspect_network("default").await.unwrap();
            assert!(text.trim_start().starts_with('['));
            assert!(text.contains("\"default\""));
            assert!(text.contains("192.168.64.0/24"));
        }
    }

    #[tokio::test]
    async fn stats_parses_cumulative_cpu_counter() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["stats", "--no-stream", "--format", "json"],
                Output::ok(fixture_version(version, "stats.json")),
            );
            let client = client_with(mock);

            let stats = client.stats().await.unwrap();

            assert_eq!(stats[0].id, "qtest");
            assert_eq!(stats[0].cpu_usage_usec, 35372);
            assert_eq!(stats[0].memory_usage_bytes, 4780032);
            assert_eq!(stats[0].network_rx_bytes, 29461);
            assert_eq!(stats[0].network_tx_bytes, 602);
            assert_eq!(stats[0].block_read_bytes, 3981312);
            assert_eq!(stats[0].block_write_bytes, 0);
        }
    }

    #[tokio::test]
    async fn system_status_running_fixture_parses() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["system", "status", "--format", "json"],
                Output::ok(fixture_version(version, "system_status_running.json")),
            );
            let client = client_with(mock);

            assert!(
                client.system_status().await.unwrap().is_running(),
                "fixture generation {version}"
            );
        }
    }

    #[tokio::test]
    async fn system_status_down_exits_1_with_json_and_maps_to_service_down() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["system", "status", "--format", "json"],
                Output {
                    code: 1,
                    stdout: fixture_version(version, "system_status_down.json"),
                    stderr: Vec::new(),
                },
            );
            let client = client_with(mock);

            let err = client.system_status().await.unwrap_err();
            assert!(
                matches!(err, CliError::ServiceDown { .. }),
                "fixture generation {version}: {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn system_status_unknown_failure_is_not_reported_as_service_down() {
        let mock = MockRunner::new();
        mock.on(
            &["system", "status", "--format", "json"],
            Output {
                code: 1,
                stdout: b"migration required\n".to_vec(),
                stderr: b"Error: authorization denied\n".to_vec(),
            },
        );
        let client = client_with(mock);

        let err = client.system_status().await.unwrap_err();

        assert!(matches!(err, CliError::Other { .. }), "{err:?}");
        assert_eq!(err.raw(), "migration required\nError: authorization denied");
    }

    #[tokio::test]
    async fn system_status_and_read_agree_that_xpc_stderr_is_service_down() {
        for version in FIXTURE_VERSIONS {
            let stderr = fixture_version(version, "stderr/service_down_ls.txt");

            let status_mock = MockRunner::new();
            status_mock.on(
                &["system", "status", "--format", "json"],
                Output::fail(1, stderr.clone()),
            );
            let status_err = client_with(status_mock).system_status().await.unwrap_err();

            let read_mock = MockRunner::new();
            read_mock.on(
                &["ls", "-a", "--format", "json"],
                Output::fail(1, stderr.clone()),
            );
            let read_err = client_with(read_mock).list_containers().await.unwrap_err();

            assert!(
                matches!(status_err, CliError::ServiceDown { .. }),
                "fixture generation {version}: {status_err:?}"
            );
            assert_eq!(status_err, read_err, "fixture generation {version}");
            assert_eq!(status_err.raw(), String::from_utf8_lossy(&stderr).trim());
        }
    }

    #[tokio::test]
    async fn read_failure_with_xpc_stderr_classifies_as_service_down() {
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["ls", "-a", "--format", "json"],
                Output::fail(1, fixture_version(version, "stderr/service_down_ls.txt")),
            );
            let client = client_with(mock);

            let err = client.list_containers().await.unwrap_err();
            assert!(
                matches!(err, CliError::ServiceDown { .. }),
                "fixture generation {version}: {err:?}"
            );
        }
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
        for version in FIXTURE_VERSIONS {
            let mock = MockRunner::new();
            mock.on(
                &["inspect", "qtest"],
                Output::ok(fixture_version(version, "inspect.json")),
            );
            let client = client_with(mock);

            let text = client.inspect_container("qtest").await.unwrap();
            assert!(text.trim_start().starts_with('['));
            assert!(text.contains("\"qtest\""));
        }
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
            Client::<MockRunner>::create_volume_args("scratch").join(" "),
            "volume create scratch"
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
            Client::<MockRunner>::tag_image_args("docker.io/library/alpine:latest", "myapp:v1")
                .join(" "),
            "image tag docker.io/library/alpine:latest myapp:v1"
        );
        assert_eq!(
            preview(&[
                "image",
                "tag",
                "docker.io/library/alpine:latest",
                "myapp:v1"
            ]),
            "container image tag docker.io/library/alpine:latest myapp:v1"
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
        let list: Vec<ContainerJson> = serde_json::from_slice(&real("ls_mounts.json")).unwrap();
        let fixture = list.iter().find(|c| c.id == "bushel-fixture").unwrap();
        assert_eq!(
            fixture.volume_sources().collect::<Vec<_>>(),
            vec!["bushel-fixture-vol"]
        );
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
        assert_eq!(list[0].network_rx_bytes, 29236);
        assert_eq!(list[0].network_tx_bytes, 602);
        assert_eq!(list[0].block_read_bytes, 4243456);
        assert_eq!(list[0].block_write_bytes, 0);
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

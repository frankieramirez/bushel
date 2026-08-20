//! Typed views of the CLI's JSON output. Shapes are only patch-stable, so every
//! struct tolerates unknown fields (serde's default — never `deny_unknown_fields`)
//! and non-essential fields are `Option` or defaulted.

use serde::Deserialize;

/// One element of `container ls -a --format json`.
#[derive(Debug, Clone, Deserialize)]
pub struct ContainerJson {
    pub id: String,
    #[serde(default)]
    pub configuration: ContainerConfig,
    #[serde(default)]
    pub status: ContainerStatus,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContainerConfig {
    #[serde(default)]
    pub image: Option<ImageRef>,
    #[serde(default, rename = "creationDate")]
    pub creation_date: Option<String>,
    #[serde(default)]
    pub mounts: Vec<Mount>,
    #[serde(default)]
    pub resources: Option<Resources>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageRef {
    pub reference: String,
}

/// The CLI's mount JSON. bushel's vocabulary is "volume" — this type exists only
/// to read which volumes a container references (the in-use badge).
#[derive(Debug, Clone, Deserialize)]
pub struct Mount {
    /// Volume name for volume mounts, host path for binds.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Resources {
    #[serde(default)]
    pub cpus: Option<u32>,
    #[serde(default, rename = "memoryInBytes")]
    pub memory_in_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContainerStatus {
    /// "running" / "stopped" — compared loosely; unknown states render verbatim.
    #[serde(default)]
    pub state: String,
    #[serde(default, rename = "startedDate")]
    pub started_date: Option<String>,
}

impl ContainerJson {
    pub fn is_running(&self) -> bool {
        self.status.state == "running"
    }

    pub fn image_reference(&self) -> &str {
        self.configuration
            .image
            .as_ref()
            .map(|i| i.reference.as_str())
            .unwrap_or("")
    }

    /// Names of volumes this container references (for the in-use badge).
    pub fn volume_sources(&self) -> impl Iterator<Item = &str> {
        self.configuration
            .mounts
            .iter()
            .filter(|m| m.kind.as_deref() == Some("volume"))
            .filter_map(|m| m.source.as_deref())
    }
}

/// One element of `container image ls --format json`.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageJson {
    pub id: String,
    #[serde(default)]
    pub configuration: ImageConfig,
    #[serde(default)]
    pub variants: Vec<ImageVariant>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ImageConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "creationDate")]
    pub creation_date: Option<String>,
    #[serde(default)]
    pub descriptor: Option<Descriptor>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Descriptor {
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageVariant {
    #[serde(default)]
    pub config: Option<VariantConfig>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VariantConfig {
    #[serde(default)]
    pub architecture: Option<String>,
    #[serde(default)]
    pub os: Option<String>,
}

impl ImageJson {
    pub fn reference(&self) -> &str {
        self.configuration.name.as_deref().unwrap_or(&self.id)
    }

    /// Size of the best-matching real variant: prefer the host platform, else any
    /// variant whose os isn't "unknown" (attestation manifests are os=unknown).
    pub fn display_size(&self) -> Option<u64> {
        let real = |v: &&ImageVariant| {
            v.config
                .as_ref()
                .and_then(|c| c.os.as_deref())
                .is_some_and(|os| os != "unknown")
        };
        let host = self.variants.iter().filter(real).find(|v| {
            v.config.as_ref().and_then(|c| c.architecture.as_deref())
                == Some(std::env::consts::ARCH)
        });
        host.or_else(|| self.variants.iter().find(real))
            .and_then(|v| v.size)
    }
}

/// One element of `container volume ls --format json`.
#[derive(Debug, Clone, Deserialize)]
pub struct VolumeJson {
    pub id: String,
    #[serde(default)]
    pub configuration: VolumeConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VolumeConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub driver: Option<String>,
    #[serde(default, rename = "creationDate")]
    pub creation_date: Option<String>,
}

impl VolumeJson {
    pub fn name(&self) -> &str {
        self.configuration.name.as_deref().unwrap_or(&self.id)
    }
}

/// One element of `container stats --no-stream --format json`.
/// `cpu_usage_usec` is cumulative — CPU% needs two samples.
#[derive(Debug, Clone, Deserialize)]
pub struct StatsJson {
    pub id: String,
    #[serde(default, rename = "cpuUsageUsec")]
    pub cpu_usage_usec: u64,
    #[serde(default, rename = "memoryUsageBytes")]
    pub memory_usage_bytes: u64,
    #[serde(default, rename = "memoryLimitBytes")]
    pub memory_limit_bytes: u64,
}

/// `container system status --format json` — the only object-shaped response.
#[derive(Debug, Clone, Deserialize)]
pub struct SystemStatusJson {
    #[serde(default)]
    pub status: String,
}

impl SystemStatusJson {
    pub fn is_running(&self) -> bool {
        self.status == "running"
    }
}

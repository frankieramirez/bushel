use std::collections::HashMap;

use serde::Deserialize;

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

#[derive(Debug, Clone, Deserialize)]
pub struct Mount {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<MountKind>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MountKind {
    #[serde(default)]
    pub volume: Option<VolumeMountInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolumeMountInfo {
    #[serde(default)]
    pub name: Option<String>,
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
    #[serde(default)]
    pub state: String,
    #[serde(default, rename = "startedDate")]
    pub started_date: Option<String>,
    #[serde(default)]
    pub networks: Vec<ContainerNetwork>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerNetwork {
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default, rename = "ipv4Address")]
    pub ipv4_address: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
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

    pub fn volume_sources(&self) -> impl Iterator<Item = &str> {
        self.configuration.mounts.iter().filter_map(|m| {
            m.kind
                .as_ref()
                .and_then(|k| k.volume.as_ref())
                .and_then(|v| v.name.as_deref())
        })
    }

    pub fn network_attachments(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.status.networks.iter().filter_map(|n| {
            n.network
                .as_deref()
                .map(|name| (name, n.ipv4_address.as_deref()))
        })
    }
}

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

const BUILTIN_ROLE_KEY: &str = "com.apple.container.resource.role";
const BUILTIN_ROLE_VALUE: &str = "builtin";

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkJson {
    pub id: String,
    #[serde(default)]
    pub configuration: NetworkConfig,
    #[serde(default)]
    pub status: NetworkStatusJson,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NetworkConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "creationDate")]
    pub creation_date: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub plugin: Option<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default, rename = "ipv4Subnet")]
    pub ipv4_subnet: Option<String>,
    #[serde(default)]
    pub subnet: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NetworkStatusJson {
    #[serde(default, rename = "ipv4Subnet")]
    pub ipv4_subnet: Option<String>,
    #[serde(default, rename = "ipv4Gateway")]
    pub ipv4_gateway: Option<String>,
    #[serde(default, rename = "ipv6Subnet")]
    pub ipv6_subnet: Option<String>,
}

impl NetworkJson {
    pub fn name(&self) -> &str {
        self.configuration.name.as_deref().unwrap_or(&self.id)
    }

    pub fn mode(&self) -> &str {
        self.configuration.mode.as_deref().unwrap_or("")
    }

    pub fn ipv4_subnet(&self) -> Option<&str> {
        self.status
            .ipv4_subnet
            .as_deref()
            .or(self.configuration.ipv4_subnet.as_deref())
            .or(self.configuration.subnet.as_deref())
    }

    pub fn is_builtin(&self) -> bool {
        self.configuration
            .labels
            .get(BUILTIN_ROLE_KEY)
            .is_some_and(|v| v == BUILTIN_ROLE_VALUE)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatsJson {
    pub id: String,
    #[serde(default, rename = "cpuUsageUsec")]
    pub cpu_usage_usec: u64,
    #[serde(default, rename = "memoryUsageBytes")]
    pub memory_usage_bytes: u64,
    #[serde(default, rename = "memoryLimitBytes")]
    pub memory_limit_bytes: u64,
    #[serde(default, rename = "networkRxBytes")]
    pub network_rx_bytes: u64,
    #[serde(default, rename = "networkTxBytes")]
    pub network_tx_bytes: u64,
    #[serde(default, rename = "blockReadBytes")]
    pub block_read_bytes: u64,
    #[serde(default, rename = "blockWriteBytes")]
    pub block_write_bytes: u64,
}

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

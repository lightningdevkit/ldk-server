use std::path::{Path, PathBuf};

use serde::Deserialize;

/// GUI-specific config extracted from ldk-server config file.
#[derive(Debug, Clone, Default)]
pub struct GuiConfig {
    pub server_url: String,
    pub api_key: String,
    pub tls_cert_path: String,
    pub network: String,
    pub chain_source: ChainSourceConfig,
}

/// Chain source configuration (Bitcoind RPC, Electrum, or Esplora)
#[derive(Debug, Clone, Default)]
pub enum ChainSourceConfig {
    #[default]
    None,
    Bitcoind {
        rpc_address: String,
        rpc_user: String,
        rpc_password: String,
    },
    Electrum {
        server_url: String,
    },
    Esplora {
        server_url: String,
    },
}

/// Partial deserialization of the ldk-server config file.
/// We only need the fields relevant to connecting as a client.
#[derive(Deserialize)]
struct TomlConfig {
    node: NodeConfig,
    storage: StorageConfig,
    bitcoind: Option<BitcoindConfig>,
    electrum: Option<ElectrumConfig>,
    esplora: Option<EsploraConfig>,
}

#[derive(Deserialize)]
struct NodeConfig {
    network: String,
    rest_service_address: String,
    api_key: String,
}

#[derive(Deserialize)]
struct StorageConfig {
    disk: DiskConfig,
}

#[derive(Deserialize)]
struct DiskConfig {
    dir_path: String,
}

#[derive(Deserialize)]
struct BitcoindConfig {
    rpc_address: String,
    rpc_user: String,
    rpc_password: String,
}

#[derive(Deserialize)]
struct ElectrumConfig {
    server_url: String,
}

#[derive(Deserialize)]
struct EsploraConfig {
    server_url: String,
}

impl TryFrom<TomlConfig> for GuiConfig {
    type Error = String;

    fn try_from(toml: TomlConfig) -> Result<Self, Self::Error> {
        let storage_dir = PathBuf::from(&toml.storage.disk.dir_path);
        let tls_cert_path = storage_dir.join("tls.crt");

        let chain_source = if let Some(btc) = toml.bitcoind {
            ChainSourceConfig::Bitcoind {
                rpc_address: btc.rpc_address,
                rpc_user: btc.rpc_user,
                rpc_password: btc.rpc_password,
            }
        } else if let Some(electrum) = toml.electrum {
            ChainSourceConfig::Electrum { server_url: electrum.server_url }
        } else if let Some(esplora) = toml.esplora {
            ChainSourceConfig::Esplora { server_url: esplora.server_url }
        } else {
            ChainSourceConfig::None
        };

        Ok(GuiConfig {
            server_url: toml.node.rest_service_address,
            api_key: toml.node.api_key,
            tls_cert_path: tls_cert_path.to_string_lossy().to_string(),
            network: toml.node.network,
            chain_source,
        })
    }
}

/// Try to load config from a file path.
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<GuiConfig, String> {
    let contents = std::fs::read_to_string(path.as_ref())
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let toml_config: TomlConfig =
        toml::from_str(&contents).map_err(|e| format!("Failed to parse config file: {}", e))?;

    GuiConfig::try_from(toml_config)
}

/// Search for config file in common locations and load it.
/// Returns None if no config file is found.
pub fn find_and_load_config() -> Option<GuiConfig> {
    let search_paths = [
        // Current directory
        PathBuf::from("ldk-server-config.toml"),
        // Parent directory (if running from ldk-server-gui)
        PathBuf::from("../ldk-server/ldk-server-config.toml"),
        // Sibling ldk-server directory
        PathBuf::from("../ldk-server-config.toml"),
    ];

    for path in &search_paths {
        if path.exists() {
            if let Ok(config) = load_config(path) {
                return Some(config);
            }
        }
    }

    // Also check if there's an environment variable pointing to config
    if let Ok(env_path) = std::env::var("LDK_SERVER_CONFIG") {
        let path = PathBuf::from(env_path);
        if path.exists() {
            if let Ok(config) = load_config(&path) {
                return Some(config);
            }
        }
    }

    None
}

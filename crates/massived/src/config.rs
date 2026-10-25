//! Configuration file.
//!
//! The file describes what the server advertises and serves. It is checked
//! completely at load time, see [`crate::content::Content::load`].

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub protocol: ProtocolConfig,
    #[serde(default)]
    pub locate: LocateConfig,
    #[serde(default)]
    pub capture: Option<CaptureConfig>,
    pub content: ContentConfig,
    #[serde(default, rename = "zone")]
    pub zones: Vec<ZoneConfig>,
    #[serde(default, rename = "asset")]
    pub assets: Vec<AssetConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub listen: Vec<SocketAddr>,
    /// Host name or address placed in the locate response for every service.
    pub advertise_host: String,
    pub advertise_port: u16,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_seconds: u64,
    /// Close the connection after each response. Disable to observe whether
    /// the client reuses connections.
    #[serde(default = "default_true")]
    pub close_after_response: bool,
}

fn default_idle_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolConfig {
    #[serde(default)]
    pub mac_policy: MacPolicy,
    #[serde(default)]
    pub server_time: ServerTimeSource,
    /// Key for request verification and response signing. Absent means the
    /// retail literal, which is what the client verifies responses with.
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MacPolicy {
    /// Answer requests with a bad MAC with HTTP 400.
    #[default]
    Enforce,
    /// Log a bad MAC and process the request.
    Report,
}

/// Value of the locate field `0x3B`, whose unit is unknown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServerTimeSource {
    #[default]
    UnixMilliseconds,
    UnixSeconds,
    Fixed(u64),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocateConfig {
    pub auto_reporting_time_seconds: Option<u16>,
    pub server_options: Option<u16>,
    pub statistic_period_minutes: Option<u8>,
    pub heartbeat_period_minutes: Option<u8>,
    pub logging_level: Option<u8>,
    pub value_57: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureConfig {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentConfig {
    /// Asset files are resolved relative to this directory.
    pub directory: PathBuf,
    /// Placeholders: `{host}`, `{port}`, `{asset_id}`.
    #[serde(default = "default_url_template")]
    pub url_template: String,
    /// Zones announced in every OpenSession response, in this order. Zones
    /// configured below that are missing here are appended.
    #[serde(default = "default_announce_zones")]
    pub announce_zones: Vec<String>,
}

fn default_url_template() -> String {
    "http://{host}:{port}/assets/{asset_id}".to_owned()
}

/// `ZonesPT` and `ZonesEP` of the retail `data.lua`.
pub const RETAIL_ZONES: [&str; 14] = [
    "Lobby_PT",
    "CommercialBreak1",
    "CommercialBreak2",
    "CommercialBreak3",
    "CommercialBreak4",
    "CommercialBreak5",
    "CommercialBreak6",
    "CommercialBreak7",
    "CommercialBreak8",
    "Lobby_EP",
    "CommercialBreakEP_1",
    "CommercialBreakEP_2",
    "CommercialBreakEP_3",
    "CommercialBreakEP_4",
];

fn default_announce_zones() -> Vec<String> {
    RETAIL_ZONES.iter().map(|&z| z.to_owned()).collect()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneConfig {
    pub name: String,
    #[serde(default, rename = "placement")]
    pub placements: Vec<PlacementConfig>,
    #[serde(default, rename = "order")]
    pub orders: Vec<OrderConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementConfig {
    pub ie_name: String,
    pub ie_id: u32,
    pub media_type: u32,
    #[serde(default)]
    pub rotation_type: u16,
    #[serde(default = "default_match_list_size")]
    pub primary_match_list_size: u8,
    pub assets: Vec<u32>,
    pub angle_threshold: Option<f32>,
    pub distance_threshold: Option<f32>,
    pub size_threshold: Option<u16>,
}

fn default_match_list_size() -> u8 {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderConfig {
    pub order_id: u32,
    pub frequency_cap: Option<u32>,
    pub assets: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetConfig {
    pub id: u32,
    pub file: PathBuf,
    pub asset_type: u32,
    /// Asset field `0x06`; must be non-zero when `crex_id` is non-zero.
    #[serde(default = "default_one")]
    pub rotation_duration: u32,
    #[serde(default = "default_expiration")]
    pub expiration: u64,
    #[serde(default = "default_one")]
    pub crex_id: u32,
    pub angle_threshold: Option<f32>,
    pub distance_threshold: Option<f32>,
    pub size_threshold: Option<u16>,
    pub extra: Option<String>,
}

fn default_one() -> u32 {
    1
}

fn default_expiration() -> u64 {
    1
}

#[derive(Debug, Error)]
pub enum ConfigFileError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
}

impl Config {
    /// Reads a configuration file. Relative paths in it are resolved against
    /// the directory that contains the file.
    pub fn from_file(path: &Path) -> Result<Self, ConfigFileError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigFileError::Read {
            path: path.to_owned(),
            source,
        })?;
        let mut config: Self = toml::from_str(&text).map_err(|source| ConfigFileError::Parse {
            path: path.to_owned(),
            source: Box::new(source),
        })?;
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        config.content.directory = base.join(&config.content.directory);
        if let Some(capture) = &mut config.capture {
            capture.path = base.join(&capture.path);
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_parses() {
        let text = include_str!("../massived.example.toml");
        let config: Config = toml::from_str(text).unwrap();
        assert!(!config.zones.is_empty());
        assert_eq!(config.protocol.mac_policy, MacPolicy::Enforce);
    }

    #[test]
    fn server_time_forms() {
        #[derive(Deserialize)]
        struct T {
            v: ServerTimeSource,
        }
        let t: T = toml::from_str("v = \"unix-seconds\"").unwrap();
        assert_eq!(t.v, ServerTimeSource::UnixSeconds);
        let t: T = toml::from_str("v = { fixed = 7 }").unwrap();
        assert_eq!(t.v, ServerTimeSource::Fixed(7));
    }
}

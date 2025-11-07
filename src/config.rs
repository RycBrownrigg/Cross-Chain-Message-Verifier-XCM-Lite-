use config::{Config, ConfigError as RawConfigError, Environment, File};
use serde::Deserialize;
use thiserror::Error;

/// Result alias for configuration loading.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Errors that can occur while loading or normalizing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(transparent)]
    Source(#[from] RawConfigError),
    #[error("invalid configuration: {0}")]
    Invalid(String),
}

/// Root configuration for the XCM Lite service.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub parachains: ParachainConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            parachains: ParachainConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load configuration values from files and environment variables.
    pub fn load() -> Result<Self> {
        let builder = Config::builder()
            .set_default("server.host", ServerConfig::default().host)?
            .set_default("server.port", ServerConfig::default().port)?
            .set_default("parachains.count", ParachainConfig::default().count)?
            .set_default(
                "parachains.xcm_version",
                ParachainConfig::default().xcm_version,
            )?
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/local").required(false))
            .add_source(Environment::with_prefix("XCM_LITE").separator("__"));

        let config = builder.build()?;
        let mut parsed: AppConfig = config.try_deserialize()?;
        parsed.normalize()?;
        Ok(parsed)
    }

    fn normalize(&mut self) -> Result<()> {
        self.parachains.normalize()?;
        Ok(())
    }
}

/// HTTP server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_owned(),
            port: 8080,
        }
    }
}

/// Configuration for the simulated parachain environment.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ParachainConfig {
    pub count: u32,
    pub xcm_version: String,
    #[serde(default)]
    pub keys: Vec<ParachainKeyConfig>,
}

impl Default for ParachainConfig {
    fn default() -> Self {
        Self {
            count: 3,
            xcm_version: "V3".to_owned(),
            keys: Vec::new(),
        }
    }
}

impl ParachainConfig {
    fn normalize(&mut self) -> Result<()> {
        if !self.keys.is_empty() {
            let mut seen = std::collections::HashSet::new();
            for key in &self.keys {
                if !seen.insert(key.para_id) {
                    return Err(ConfigError::Invalid(format!(
                        "duplicate parachain id {} in key configuration",
                        key.para_id
                    )));
                }
            }
            self.count = self.count.max(self.keys.len() as u32);
        }
        Ok(())
    }

    /// Return the list of parachain ids that should be initialised.
    pub fn parachain_ids(&self) -> Vec<u32> {
        if self.keys.is_empty() {
            (0..self.count).map(|idx| 1_000 + idx).collect()
        } else {
            self.keys.iter().map(|entry| entry.para_id).collect()
        }
    }
}

/// Configuration for pre-defined parachain keypairs.
#[derive(Debug, Clone, Deserialize)]
pub struct ParachainKeyConfig {
    pub para_id: u32,
    pub seed_phrase: Option<String>,
    pub secret_key: Option<String>,
}

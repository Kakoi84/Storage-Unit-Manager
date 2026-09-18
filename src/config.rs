use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

/// User-configurable application settings.
///
/// The application version is supplied by Cargo and is not written
/// into config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub company_name: String,
    pub database_file: String,

    #[serde(skip, default = "package_version")]
    pub version: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            company_name: "The Core Storage".to_string(),
            database_file: "StorageManager.db".to_string(),
            version: package_version(),
        }
    }
}

impl Config {
    /// Loads configuration from disk.
    ///
    /// If the file does not exist, a default configuration file is
    /// created and the defaults are returned.
    pub fn load_or_create<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        if !path.exists() {
            let config = Self::default();

            config.save(path).with_context(|| {
                format!(
                    "Unable to create default configuration file: {}",
                    path.display()
                )
            })?;

            return Ok(config);
        }

        let contents = fs::read_to_string(path)
            .with_context(|| format!("Unable to read configuration file: {}", path.display()))?;

        let mut config: Self = toml::from_str(&contents)
            .with_context(|| format!("Unable to parse configuration file: {}", path.display()))?;

        // Always use the version from Cargo.toml rather than a file value.
        config.version = package_version();

        Ok(config)
    }

    /// Writes the current configuration to disk.
    pub fn save<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "Unable to create configuration directory: {}",
                        parent.display()
                    )
                })?;
            }
        }

        let contents = toml::to_string_pretty(self)
            .context("Unable to serialize application configuration")?;

        fs::write(path, contents)
            .with_context(|| format!("Unable to write configuration file: {}", path.display()))?;

        Ok(())
    }
}

fn package_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

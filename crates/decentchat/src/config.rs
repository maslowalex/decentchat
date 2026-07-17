//! Configuration directory and Guardian storage paths.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const CONFIG_DIR_NAME: &str = "decentchat";
const LEGACY_IDENTITY_FILE_NAME: &str = "identity.key";
const GUARDIAN_DIR_NAME: &str = "guardian";
const HOST_DIR_NAME: &str = "host";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigRole {
    Client,
    Host,
}

pub fn config_dir(custom: Option<PathBuf>, role: ConfigRole) -> Result<PathBuf> {
    let dir = match custom {
        Some(path) => path,
        None => {
            let base = dirs::config_dir()
                .context("failed to determine config directory")?
                .join(CONFIG_DIR_NAME);
            default_config_dir(base, role)
        }
    };
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config directory: {}", dir.display()))?;
    Ok(dir)
}

fn default_config_dir(base: PathBuf, role: ConfigRole) -> PathBuf {
    match role {
        ConfigRole::Client => base,
        ConfigRole::Host => base.join(HOST_DIR_NAME),
    }
}

pub fn legacy_identity_path(config_dir: &Path) -> PathBuf {
    config_dir.join(LEGACY_IDENTITY_FILE_NAME)
}

pub fn guardian_data_dir(config_dir: &Path) -> PathBuf {
    config_dir.join(GUARDIAN_DIR_NAME)
}

pub fn guardian_identity_path(config_dir: &Path) -> PathBuf {
    guardian_data_dir(config_dir).join("node_secret.key")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_config_directory_is_never_role_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let custom = dir.path().join("custom");

        assert_eq!(
            config_dir(Some(custom.clone()), ConfigRole::Client).unwrap(),
            custom
        );
        assert_eq!(
            config_dir(Some(custom.clone()), ConfigRole::Host).unwrap(),
            custom
        );
    }

    #[test]
    fn default_host_directory_is_isolated_from_client() {
        let base = PathBuf::from("/config/decentchat");
        assert_eq!(default_config_dir(base.clone(), ConfigRole::Client), base);
        assert_eq!(
            default_config_dir(base.clone(), ConfigRole::Host),
            base.join("host")
        );
    }
}

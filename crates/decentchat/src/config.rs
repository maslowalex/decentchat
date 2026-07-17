//! Configuration directory and Guardian storage paths.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const CONFIG_DIR_NAME: &str = "decentchat";
const LEGACY_IDENTITY_FILE_NAME: &str = "identity.key";
const GUARDIAN_DIR_NAME: &str = "guardian";

pub fn config_dir(custom: Option<PathBuf>) -> Result<PathBuf> {
    let dir = match custom {
        Some(path) => path,
        None => dirs::config_dir()
            .context("failed to determine config directory")?
            .join(CONFIG_DIR_NAME),
    };
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config directory: {}", dir.display()))?;
    Ok(dir)
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

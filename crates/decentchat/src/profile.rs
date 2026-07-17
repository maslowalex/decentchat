//! Persistent client preferences.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const PROFILE_FILE_NAME: &str = "profile.json";
const PROFILE_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct Profile {
    version: u32,
    display_name: String,
}

pub fn load_display_name(config_dir: &Path) -> Result<Option<String>> {
    let path = profile_path(config_dir);
    if !path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(&path)
        .with_context(|| format!("failed to read client profile: {}", path.display()))?;
    let profile: Profile = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "invalid client profile: {}; fix or remove this file",
            path.display()
        )
    })?;
    if profile.version != PROFILE_VERSION {
        bail!(
            "unsupported client profile version {} in {}; upgrade DecentChat or remove this file",
            profile.version,
            path.display()
        );
    }

    Ok(Some(normalize_display_name(&profile.display_name)?))
}

pub fn save_display_name(config_dir: &Path, display_name: &str) -> Result<String> {
    let display_name = normalize_display_name(display_name)?;
    let profile = Profile {
        version: PROFILE_VERSION,
        display_name: display_name.clone(),
    };
    let mut bytes =
        serde_json::to_vec_pretty(&profile).context("failed to encode client profile")?;
    bytes.push(b'\n');

    fs::create_dir_all(config_dir).with_context(|| {
        format!(
            "failed to create client config directory: {}",
            config_dir.display()
        )
    })?;
    let path = profile_path(config_dir);
    let temp_path = temporary_profile_path(config_dir);
    fs::write(&temp_path, bytes)
        .with_context(|| format!("failed to write client profile: {}", temp_path.display()))?;
    fs::rename(&temp_path, &path)
        .with_context(|| format!("failed to save client profile: {}", path.display()))?;
    Ok(display_name)
}

pub fn normalize_display_name(display_name: &str) -> Result<String> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        bail!("display name must not be empty");
    }
    Ok(display_name.to_owned())
}

fn profile_path(config_dir: &Path) -> PathBuf {
    config_dir.join(PROFILE_FILE_NAME)
}

fn temporary_profile_path(config_dir: &Path) -> PathBuf {
    config_dir.join(format!("{PROFILE_FILE_NAME}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_loads_and_trims_display_name() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_display_name(dir.path()).unwrap(), None);

        assert_eq!(save_display_name(dir.path(), "  Alice  ").unwrap(), "Alice");
        assert_eq!(
            load_display_name(dir.path()).unwrap(),
            Some("Alice".to_owned())
        );
    }

    #[test]
    fn rejects_empty_and_unsupported_profiles() {
        let dir = tempfile::tempdir().unwrap();
        assert!(save_display_name(dir.path(), "   ").is_err());

        fs::write(
            profile_path(dir.path()),
            r#"{"version":2,"display_name":"Alice"}"#,
        )
        .unwrap();
        let error = load_display_name(dir.path()).unwrap_err().to_string();
        assert!(error.contains("unsupported client profile version"));
    }

    #[test]
    fn reports_corrupt_profile() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(profile_path(dir.path()), "not json").unwrap();
        let error = load_display_name(dir.path()).unwrap_err().to_string();
        assert!(error.contains("invalid client profile"));
    }
}

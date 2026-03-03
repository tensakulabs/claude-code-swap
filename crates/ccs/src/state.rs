use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::config::{config_dir, ensure_config_dir};
use crate::error::CcsError;

#[derive(Debug, Serialize, Deserialize, Default)]
struct State {
    #[serde(default)]
    active: Option<String>,
}

/// Returns the path to state.yaml.
pub fn state_file() -> PathBuf {
    config_dir().join("state.yaml")
}

/// Returns the active profile name, defaulting to "default" if missing/corrupt.
pub fn get_active_profile() -> String {
    let path = state_file();
    if !path.exists() {
        return "default".to_string();
    }
    match fs::read_to_string(&path) {
        Ok(content) => {
            let state: State = serde_yaml::from_str(&content).unwrap_or_default();
            state
                .active
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "default".to_string())
        }
        Err(_) => "default".to_string(),
    }
}

/// Atomically write the active profile name to state.yaml.
pub fn set_active_profile(name: &str) -> Result<(), CcsError> {
    ensure_config_dir()?;
    let path = state_file();
    let state = State {
        active: Some(name.to_string()),
    };
    let content = serde_yaml::to_string(&state)?;
    let tmp = path.with_extension("yaml.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
    }
    #[cfg(unix)]
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_active_profile_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        assert_eq!(get_active_profile(), "default");
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_get_active_profile_reads_stored() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("state.yaml"), "active: ollama\n").unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        assert_eq!(get_active_profile(), "ollama");
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_get_active_profile_fallback_on_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("state.yaml"), ": bad yaml [").unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        assert_eq!(get_active_profile(), "default");
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_get_active_profile_fallback_on_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("state.yaml"), "").unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        assert_eq!(get_active_profile(), "default");
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_set_active_profile_writes() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        set_active_profile("openrouter").unwrap();
        assert_eq!(get_active_profile(), "openrouter");
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    #[cfg(unix)]
    fn test_set_active_profile_permissions() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        set_active_profile("ollama").unwrap();
        let path = dir.path().join("state.yaml");
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_set_active_profile_no_tmp_remains() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        set_active_profile("gemini").unwrap();
        let tmp = dir.path().join("state.yaml.tmp");
        assert!(!tmp.exists());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_set_active_profile_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        set_active_profile("ollama").unwrap();
        set_active_profile("openrouter").unwrap();
        assert_eq!(get_active_profile(), "openrouter");
        std::env::remove_var("CCS_CONFIG_DIR");
    }
}

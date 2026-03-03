use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::color;
use crate::error::CcsError;

/// Returns the config directory, respecting CCS_CONFIG_DIR override.
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CCS_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".claude-code-swap")
}

/// Returns the path to config.yaml.
pub fn config_file() -> PathBuf {
    config_dir().join("config.yaml")
}

/// Create the config directory with mode 0700.
pub fn ensure_config_dir() -> Result<(), CcsError> {
    let dir = config_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Profile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Models>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    /// Catch-all for unknown fields (forward compatibility).
    #[serde(flatten, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Models {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub haiku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sonnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opus: Option<String>,
}

// ---------------------------------------------------------------------------
// Config I/O
// ---------------------------------------------------------------------------

/// Load config.yaml. Returns empty Config if missing or empty.
pub fn load_config() -> Result<Config, CcsError> {
    let path = config_file();
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = fs::read_to_string(&path)?;
    if content.trim().is_empty() {
        return Ok(Config::default());
    }
    let config: Config = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// Atomically write config.yaml with mode 0600.
pub fn save_config(config: &Config) -> Result<(), CcsError> {
    save_config_to(config, &config_file())
}

/// Atomically write config to a specific path with mode 0600.
pub fn save_config_to(config: &Config, path: &Path) -> Result<(), CcsError> {
    ensure_config_dir()?;
    let content = serde_yaml::to_string(config)?;
    let tmp = path.with_extension("yaml.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
    }
    #[cfg(unix)]
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Environment variable resolution
// ---------------------------------------------------------------------------

/// Expand ${VAR} references from the environment.
pub fn resolve_env_refs(value: &str) -> Result<String, CcsError> {
    let re = Regex::new(r"\$\{([^}]+)\}").unwrap();
    let mut result = value.to_string();
    // Collect matches first to avoid borrow issues.
    let captures: Vec<(String, String)> = re
        .captures_iter(value)
        .map(|cap| (cap[0].to_string(), cap[1].to_string()))
        .collect();
    for (full_match, var_name) in captures {
        match std::env::var(&var_name) {
            Ok(val) => result = result.replace(&full_match, &val),
            Err(_) => return Err(CcsError::EnvVar { var: var_name }),
        }
    }
    Ok(result)
}

/// Resolve all string fields in a Profile, expanding ${VAR} references.
pub fn resolve_profile_env(profile: &Profile) -> Result<Profile, CcsError> {
    let mut resolved = profile.clone();
    if let Some(ref v) = resolved.base_url {
        resolved.base_url = Some(resolve_env_refs(v)?);
    }
    if let Some(ref v) = resolved.auth_token {
        resolved.auth_token = Some(resolve_env_refs(v)?);
    }
    if let Some(ref v) = resolved.api_key {
        resolved.api_key = Some(resolve_env_refs(v)?);
    }
    if let Some(ref models) = resolved.models {
        let mut m = models.clone();
        if let Some(ref v) = m.haiku {
            m.haiku = Some(resolve_env_refs(v)?);
        }
        if let Some(ref v) = m.sonnet {
            m.sonnet = Some(resolve_env_refs(v)?);
        }
        if let Some(ref v) = m.opus {
            m.opus = Some(resolve_env_refs(v)?);
        }
        resolved.models = Some(m);
    }
    // Resolve env passthrough values
    if let Some(ref env_map) = resolved.env {
        let mut new_env = BTreeMap::new();
        for (k, v) in env_map {
            new_env.insert(k.clone(), resolve_env_refs(v)?);
        }
        resolved.env = Some(new_env);
    }
    Ok(resolved)
}

/// Convert a resolved profile to ANTHROPIC_* environment variable overrides.
pub fn build_env_overrides(profile: &Profile) -> BTreeMap<String, String> {
    let mut overrides = BTreeMap::new();

    if let Some(ref val) = profile.base_url {
        overrides.insert("ANTHROPIC_BASE_URL".into(), val.clone());
    }
    if let Some(ref val) = profile.auth_token {
        overrides.insert("ANTHROPIC_AUTH_TOKEN".into(), val.clone());
    }
    if let Some(ref val) = profile.api_key {
        overrides.insert("ANTHROPIC_API_KEY".into(), val.clone());
    }

    if let Some(ref models) = profile.models {
        if let Some(ref v) = models.haiku {
            overrides.insert("ANTHROPIC_DEFAULT_HAIKU_MODEL".to_string(), v.clone());
        }
        if let Some(ref v) = models.sonnet {
            overrides.insert("ANTHROPIC_DEFAULT_SONNET_MODEL".to_string(), v.clone());
        }
        if let Some(ref v) = models.opus {
            overrides.insert("ANTHROPIC_DEFAULT_OPUS_MODEL".to_string(), v.clone());
        }
    }

    // Pass through extra env vars
    if let Some(ref env_map) = profile.env {
        for (k, v) in env_map {
            overrides.insert(k.clone(), v.clone());
        }
    }

    overrides
}

/// Warn if API key-like strings are found directly in profile values.
pub fn warn_hardcoded_keys(config: &Config) {
    let key_pattern = Regex::new(r"\b(sk|or)-[A-Za-z0-9]{20,}\b").unwrap();
    let mut warnings = Vec::new();

    for (name, profile) in &config.profiles {
        scan_for_keys(
            &key_pattern,
            &serde_yaml::to_value(profile).unwrap_or_default(),
            &format!("profiles.{name}"),
            &mut warnings,
        );
    }

    if !warnings.is_empty() {
        let paths = warnings.join(", ");
        color::warn_msg(&format!(
            "Possible API key(s) found directly in config at: {paths}\n  \
             Use ${{ENV_VAR}} references instead of hardcoding keys."
        ));
    }
}

fn scan_for_keys(
    pattern: &Regex,
    value: &serde_yaml::Value,
    path: &str,
    warnings: &mut Vec<String>,
) {
    match value {
        serde_yaml::Value::String(s) => {
            if pattern.is_match(s) {
                warnings.push(path.to_string());
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                let key_str = k.as_str().unwrap_or("?");
                let child_path = if path.is_empty() {
                    key_str.to_string()
                } else {
                    format!("{path}.{key_str}")
                };
                scan_for_keys(pattern, v, &child_path, warnings);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for (i, v) in seq.iter().enumerate() {
                scan_for_keys(pattern, v, &format!("{path}[{i}]"), warnings);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let config = load_config().unwrap();
        assert!(config.profiles.is_empty());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_load_config_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.yaml"), "").unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let config = load_config().unwrap();
        assert!(config.profiles.is_empty());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_load_config_valid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "profiles:\n  default: {}\n  ollama:\n    base_url: http://localhost\n",
        )
        .unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let config = load_config().unwrap();
        assert!(config.profiles.contains_key("ollama"));
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_resolve_env_refs_expands() {
        std::env::set_var("CCS_TEST_KEY", "abc123");
        assert_eq!(resolve_env_refs("${CCS_TEST_KEY}").unwrap(), "abc123");
        std::env::remove_var("CCS_TEST_KEY");
    }

    #[test]
    fn test_resolve_env_refs_noop_on_plain_string() {
        assert_eq!(resolve_env_refs("plain string").unwrap(), "plain string");
    }

    #[test]
    fn test_resolve_env_refs_multiple() {
        std::env::set_var("CCS_HOST", "localhost");
        std::env::set_var("CCS_PORT", "8080");
        assert_eq!(
            resolve_env_refs("${CCS_HOST}:${CCS_PORT}").unwrap(),
            "localhost:8080"
        );
        std::env::remove_var("CCS_HOST");
        std::env::remove_var("CCS_PORT");
    }

    #[test]
    fn test_resolve_env_refs_missing_raises() {
        std::env::remove_var("CCS_MISSING_VAR");
        let result = resolve_env_refs("${CCS_MISSING_VAR}");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CCS_MISSING_VAR"));
    }

    #[test]
    fn test_build_env_overrides_full() {
        let profile = Profile {
            base_url: Some("https://example.com".into()),
            auth_token: Some("token123".into()),
            api_key: Some(String::new()),
            models: Some(Models {
                haiku: Some("fast".into()),
                sonnet: Some("big".into()),
                opus: Some("biggest".into()),
            }),
            ..Default::default()
        };
        let result = build_env_overrides(&profile);
        assert_eq!(result["ANTHROPIC_BASE_URL"], "https://example.com");
        assert_eq!(result["ANTHROPIC_AUTH_TOKEN"], "token123");
        assert_eq!(result["ANTHROPIC_API_KEY"], "");
        assert_eq!(result["ANTHROPIC_DEFAULT_HAIKU_MODEL"], "fast");
        assert_eq!(result["ANTHROPIC_DEFAULT_SONNET_MODEL"], "big");
        assert_eq!(result["ANTHROPIC_DEFAULT_OPUS_MODEL"], "biggest");
    }

    #[test]
    fn test_build_env_overrides_empty() {
        let profile = Profile::default();
        assert!(build_env_overrides(&profile).is_empty());
    }

    #[test]
    fn test_build_env_overrides_partial() {
        let profile = Profile {
            base_url: Some("http://x.com".into()),
            ..Default::default()
        };
        let result = build_env_overrides(&profile);
        assert_eq!(result["ANTHROPIC_BASE_URL"], "http://x.com");
        assert!(!result.contains_key("ANTHROPIC_AUTH_TOKEN"));
    }

    #[test]
    fn test_build_env_overrides_extra_env() {
        let mut env = BTreeMap::new();
        env.insert("OLLAMA_CONTEXT_LENGTH".into(), "64000".into());
        env.insert("MY_CUSTOM".into(), "value".into());
        let profile = Profile {
            base_url: Some("http://x".into()),
            env: Some(env),
            ..Default::default()
        };
        let result = build_env_overrides(&profile);
        assert_eq!(result["OLLAMA_CONTEXT_LENGTH"], "64000");
        assert_eq!(result["MY_CUSTOM"], "value");
    }

    #[test]
    fn test_save_config_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let mut profiles = BTreeMap::new();
        profiles.insert("default".into(), Profile::default());
        profiles.insert(
            "test".into(),
            Profile {
                base_url: Some("http://x".into()),
                ..Default::default()
            },
        );
        let config = Config { profiles };
        let path = dir.path().join("config.yaml");
        save_config_to(&config, &path).unwrap();

        let loaded = load_config().unwrap();
        assert_eq!(
            loaded.profiles["test"].base_url.as_deref(),
            Some("http://x")
        );
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    #[cfg(unix)]
    fn test_save_config_permissions() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let config = Config::default();
        let path = dir.path().join("config.yaml");
        save_config_to(&config, &path).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_warn_hardcoded_keys_detects() {
        // Just ensure it doesn't panic — output goes to stderr
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "bad".into(),
            Profile {
                auth_token: Some("sk-abcdefghijklmnopqrstuvwxyz1234567890".into()),
                ..Default::default()
            },
        );
        let config = Config { profiles };
        warn_hardcoded_keys(&config);
    }

    #[test]
    fn test_resolve_profile_env_nested() {
        std::env::set_var("CCS_OR_KEY", "key-abc");
        let profile = Profile {
            base_url: Some("https://openrouter.ai".into()),
            auth_token: Some("${CCS_OR_KEY}".into()),
            models: Some(Models {
                sonnet: Some("model-x".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let resolved = resolve_profile_env(&profile).unwrap();
        assert_eq!(resolved.auth_token.as_deref(), Some("key-abc"));
        assert_eq!(resolved.base_url.as_deref(), Some("https://openrouter.ai"));
        std::env::remove_var("CCS_OR_KEY");
    }
}

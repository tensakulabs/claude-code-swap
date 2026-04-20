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

/// Returns the path to the profiles directory.
pub fn profiles_dir() -> PathBuf {
    config_dir().join("profiles")
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

/// Create the profiles directory with mode 0700.
pub fn ensure_profiles_dir() -> Result<(), CcsError> {
    ensure_config_dir()?;
    let dir = profiles_dir();
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

/// Load profiles from individual `profiles/*.yaml` files.
/// Each file is a bare Profile (no wrapper). Filename stem = profile name.
pub fn load_profile_files() -> Result<BTreeMap<String, Profile>, CcsError> {
    let dir = profiles_dir();
    let mut profiles = BTreeMap::new();
    if !dir.exists() {
        return Ok(profiles);
    }
    let mut entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "yaml" || ext == "yml")
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        if content.trim().is_empty() {
            continue;
        }
        let profile: Profile = serde_yaml::from_str(&content).map_err(|e| {
            CcsError::Config(format!("Failed to parse profiles/{}.yaml: {e}", name))
        })?;
        profiles.insert(name, profile);
    }
    Ok(profiles)
}

/// Load config from both config.yaml (legacy) and profiles/*.yaml.
/// Profile files take precedence on name conflicts.
pub fn load_config() -> Result<Config, CcsError> {
    // Load legacy config.yaml
    let mut config = {
        let path = config_file();
        if !path.exists() {
            Config::default()
        } else {
            let content = fs::read_to_string(&path)?;
            if content.trim().is_empty() {
                Config::default()
            } else {
                serde_yaml::from_str(&content)?
            }
        }
    };

    // Overlay individual profile files (take precedence)
    let file_profiles = load_profile_files()?;
    config.profiles.extend(file_profiles);

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

/// Save a single profile to `profiles/{name}.yaml` with mode 0600.
pub fn save_profile(name: &str, profile: &Profile) -> Result<(), CcsError> {
    ensure_profiles_dir()?;
    let path = profiles_dir().join(format!("{name}.yaml"));
    let content = serde_yaml::to_string(profile)?;
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

/// Delete `profiles/{name}.yaml`. No error if the file doesn't exist.
pub fn delete_profile_file(name: &str) -> Result<(), CcsError> {
    let path = profiles_dir().join(format!("{name}.yaml"));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// Migrate config.yaml profiles into individual `profiles/*.yaml` files,
/// then remove config.yaml.
pub fn migrate_config() -> Result<usize, CcsError> {
    let cfg_path = config_file();
    if !cfg_path.exists() {
        return Err(CcsError::Config(
            "No config.yaml found — nothing to migrate".into(),
        ));
    }

    let content = fs::read_to_string(&cfg_path)?;
    if content.trim().is_empty() {
        fs::remove_file(&cfg_path)?;
        return Ok(0);
    }

    let config: Config = serde_yaml::from_str(&content)?;
    let count = config.profiles.len();

    for (name, profile) in &config.profiles {
        save_profile(name, profile)?;
    }

    fs::remove_file(&cfg_path)?;
    Ok(count)
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
    use crate::ENV_LOCK;

    #[test]
    fn test_load_config_missing_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let config = load_config().unwrap();
        assert!(config.profiles.is_empty());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_load_config_empty_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.yaml"), "").unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let config = load_config().unwrap();
        assert!(config.profiles.is_empty());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_load_config_valid_yaml() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

    // ── profile files ─────────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_profile_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let profile = Profile {
            base_url: Some("http://localhost:11434/v1".into()),
            auth_token: Some("ollama".into()),
            ..Default::default()
        };
        save_profile("ollama", &profile).unwrap();

        assert!(dir.path().join("profiles/ollama.yaml").exists());

        let loaded = load_profile_files().unwrap();
        assert_eq!(
            loaded["ollama"].base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_load_profile_files_empty_dir() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        fs::create_dir_all(dir.path().join("profiles")).unwrap();

        let loaded = load_profile_files().unwrap();
        assert!(loaded.is_empty());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_load_profile_files_no_dir() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());

        let loaded = load_profile_files().unwrap();
        assert!(loaded.is_empty());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_delete_profile_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let profile = Profile {
            base_url: Some("http://x".into()),
            ..Default::default()
        };
        save_profile("test", &profile).unwrap();
        assert!(dir.path().join("profiles/test.yaml").exists());

        delete_profile_file("test").unwrap();
        assert!(!dir.path().join("profiles/test.yaml").exists());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_delete_profile_file_nonexistent() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        // Should not error
        delete_profile_file("nope").unwrap();
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_load_config_merges_both_sources() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());

        // Write legacy config.yaml with one profile
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "legacy".into(),
            Profile {
                base_url: Some("http://legacy".into()),
                ..Default::default()
            },
        );
        let config = Config { profiles };
        let path = dir.path().join("config.yaml");
        save_config_to(&config, &path).unwrap();

        // Write a profile file
        let profile = Profile {
            base_url: Some("http://new".into()),
            ..Default::default()
        };
        save_profile("new-profile", &profile).unwrap();

        let loaded = load_config().unwrap();
        assert_eq!(
            loaded.profiles["legacy"].base_url.as_deref(),
            Some("http://legacy")
        );
        assert_eq!(
            loaded.profiles["new-profile"].base_url.as_deref(),
            Some("http://new")
        );
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_load_config_profile_file_wins_on_conflict() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());

        // Write legacy config.yaml
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "shared".into(),
            Profile {
                base_url: Some("http://old".into()),
                ..Default::default()
            },
        );
        let config = Config { profiles };
        save_config_to(&config, &dir.path().join("config.yaml")).unwrap();

        // Write profile file with same name
        save_profile(
            "shared",
            &Profile {
                base_url: Some("http://new".into()),
                ..Default::default()
            },
        )
        .unwrap();

        let loaded = load_config().unwrap();
        assert_eq!(
            loaded.profiles["shared"].base_url.as_deref(),
            Some("http://new"),
            "profile file should take precedence over config.yaml"
        );
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_migrate_config() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());

        // Write config.yaml with two profiles
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "alpha".into(),
            Profile {
                base_url: Some("http://alpha".into()),
                ..Default::default()
            },
        );
        profiles.insert(
            "beta".into(),
            Profile {
                base_url: Some("http://beta".into()),
                ..Default::default()
            },
        );
        let config = Config { profiles };
        save_config_to(&config, &dir.path().join("config.yaml")).unwrap();

        let count = migrate_config().unwrap();
        assert_eq!(count, 2);
        assert!(!dir.path().join("config.yaml").exists());
        assert!(dir.path().join("profiles/alpha.yaml").exists());
        assert!(dir.path().join("profiles/beta.yaml").exists());

        // Verify they load correctly
        let loaded = load_config().unwrap();
        assert_eq!(
            loaded.profiles["alpha"].base_url.as_deref(),
            Some("http://alpha")
        );
        assert_eq!(
            loaded.profiles["beta"].base_url.as_deref(),
            Some("http://beta")
        );
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_migrate_config_no_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        let result = migrate_config();
        assert!(result.is_err());
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    #[cfg(unix)]
    fn test_save_profile_permissions() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        save_profile("test", &Profile::default()).unwrap();

        let path = dir.path().join("profiles/test.yaml");
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::env::remove_var("CCS_CONFIG_DIR");
    }
}

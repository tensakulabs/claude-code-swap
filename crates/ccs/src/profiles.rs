use crate::config::{Config, Profile};
use crate::error::CcsError;

/// Return sorted list of profile names. Always includes "default".
pub fn list_profiles(config: &Config) -> Vec<String> {
    let mut names: Vec<String> = config.profiles.keys().cloned().collect();
    if !names.contains(&"default".to_string()) {
        names.push("default".to_string());
    }
    names.sort();
    names
}

/// Return profile data. Raises error if not found.
pub fn get_profile(config: &Config, name: &str) -> Result<Profile, CcsError> {
    if name == "default" {
        return Ok(config.profiles.get("default").cloned().unwrap_or_default());
    }
    config.profiles.get(name).cloned().ok_or_else(|| {
        let available = list_profiles(config).join(", ");
        CcsError::ProfileNotFound {
            name: name.to_string(),
            available,
        }
    })
}

/// Return a new Config with the profile added/updated.
pub fn add_profile(config: &Config, name: &str, data: Profile) -> Config {
    let mut new_config = config.clone();
    new_config.profiles.insert(name.to_string(), data);
    new_config
}

/// Return a new Config with the profile removed.
pub fn remove_profile(config: &Config, name: &str) -> Result<Config, CcsError> {
    if name == "default" {
        return Err(CcsError::Other(
            "Cannot remove the 'default' profile".to_string(),
        ));
    }
    if !config.profiles.contains_key(name) {
        return Err(CcsError::ProfileNotFound {
            name: name.to_string(),
            available: list_profiles(config).join(", "),
        });
    }
    let mut new_config = config.clone();
    new_config.profiles.remove(name);
    Ok(new_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_config() -> Config {
        let mut profiles = BTreeMap::new();
        profiles.insert("default".into(), Profile::default());
        profiles.insert(
            "ollama".into(),
            Profile {
                base_url: Some("http://localhost:11434/v1".into()),
                auth_token: Some("ollama".into()),
                ..Default::default()
            },
        );
        profiles.insert(
            "openrouter".into(),
            Profile {
                base_url: Some("https://openrouter.ai/api/v1".into()),
                auth_token: Some("${OPENROUTER_API_KEY}".into()),
                ..Default::default()
            },
        );
        Config { profiles }
    }

    #[test]
    fn test_list_profiles_sorted() {
        let config = sample_config();
        let result = list_profiles(&config);
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(result, sorted);
    }

    #[test]
    fn test_list_profiles_includes_default() {
        let mut profiles = BTreeMap::new();
        profiles.insert("ollama".into(), Profile::default());
        let config = Config { profiles };
        assert!(list_profiles(&config).contains(&"default".to_string()));
    }

    #[test]
    fn test_list_profiles_empty_config() {
        let config = Config::default();
        assert_eq!(list_profiles(&config), vec!["default".to_string()]);
    }

    #[test]
    fn test_get_profile_existing() {
        let config = sample_config();
        let profile = get_profile(&config, "ollama").unwrap();
        assert_eq!(
            profile.base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
    }

    #[test]
    fn test_get_profile_default_returns_empty_when_not_defined() {
        let mut profiles = BTreeMap::new();
        profiles.insert("ollama".into(), Profile::default());
        let config = Config { profiles };
        let profile = get_profile(&config, "default").unwrap();
        assert_eq!(profile, Profile::default());
    }

    #[test]
    fn test_get_profile_default_explicit() {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "default".into(),
            Profile {
                base_url: Some("http://x".into()),
                ..Default::default()
            },
        );
        let config = Config { profiles };
        let profile = get_profile(&config, "default").unwrap();
        assert_eq!(profile.base_url.as_deref(), Some("http://x"));
    }

    #[test]
    fn test_get_profile_not_found() {
        let config = sample_config();
        let result = get_profile(&config, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
        assert!(err.contains("default"));
        assert!(err.contains("ollama"));
    }

    #[test]
    fn test_add_profile_new() {
        let config = Config {
            profiles: BTreeMap::from([("default".into(), Profile::default())]),
        };
        let data = Profile {
            base_url: Some("http://new".into()),
            ..Default::default()
        };
        let result = add_profile(&config, "new-profile", data);
        assert!(result.profiles.contains_key("new-profile"));
        // Original not mutated
        assert!(!config.profiles.contains_key("new-profile"));
    }

    #[test]
    fn test_add_profile_updates_existing() {
        let config = Config {
            profiles: BTreeMap::from([(
                "ollama".into(),
                Profile {
                    base_url: Some("old".into()),
                    ..Default::default()
                },
            )]),
        };
        let result = add_profile(
            &config,
            "ollama",
            Profile {
                base_url: Some("new".into()),
                ..Default::default()
            },
        );
        assert_eq!(result.profiles["ollama"].base_url.as_deref(), Some("new"));
    }

    #[test]
    fn test_remove_profile() {
        let config = sample_config();
        let result = remove_profile(&config, "ollama").unwrap();
        assert!(!result.profiles.contains_key("ollama"));
        // Original not mutated
        assert!(config.profiles.contains_key("ollama"));
    }

    #[test]
    fn test_remove_profile_default_rejected() {
        let config = sample_config();
        let result = remove_profile(&config, "default");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("default"));
    }

    #[test]
    fn test_remove_profile_not_found() {
        let config = sample_config();
        let result = remove_profile(&config, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}

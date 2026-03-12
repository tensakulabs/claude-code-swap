use std::collections::BTreeMap;

use crate::config::{build_env_overrides, resolve_profile_env, Profile};
use crate::error::CcsError;

/// Find the claude binary.
///
/// Search order:
///   1. CLAUDE_BINARY env var (for testing/overrides)
///   2. `which` lookup (uses current PATH)
///   3. Common npm/node install fallback paths
///   4. Error with install hint
pub fn find_claude_binary() -> Result<String, CcsError> {
    if let Ok(override_path) = std::env::var("CLAUDE_BINARY") {
        return Ok(override_path);
    }
    if let Ok(path) = which::which("claude") {
        return Ok(path.to_string_lossy().to_string());
    }
    // PATH may be incomplete (e.g., nvm not sourced, non-login shell).
    // Check common npm/node install locations before giving up.
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates: &[String] = &[
        "/usr/local/bin/claude".to_string(),
        "/opt/homebrew/bin/claude".to_string(),
        format!("{home}/.npm/bin/claude"),
        format!("{home}/.npm-global/bin/claude"),
        format!("{home}/.local/bin/claude"),
        "/usr/bin/claude".to_string(),
    ];
    for path_str in candidates {
        let p = std::path::Path::new(path_str.as_str());
        if p.exists() {
            return Ok(path_str.clone());
        }
    }
    Err(CcsError::BinaryNotFound)
}

/// ANTHROPIC_* keys that ccs manages. These are always stripped from the
/// inherited shell environment before launching, then re-applied from the
/// active profile. This prevents shell-level vars from bleeding through
/// when the `default` profile (or any profile without that key) is active.
const MANAGED_ENV_KEYS: &[&str] = &[
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
];

/// Build the full environment for exec.
///
/// Strips all managed ANTHROPIC_* vars from the inherited shell environment,
/// then re-applies only what the active profile explicitly sets.
///
/// This ensures `default` profile truly means "Anthropic subscription" even
/// if the user has ANTHROPIC_BASE_URL set in their shell.
pub fn build_env(resolved_profile: &Profile) -> BTreeMap<String, String> {
    let mut env: BTreeMap<String, String> = std::env::vars().collect();

    // Strip managed keys — profile values (or absence) take full ownership.
    for key in MANAGED_ENV_KEYS {
        env.remove(*key);
    }

    // Re-apply only what the active profile explicitly sets.
    let overrides = build_env_overrides(resolved_profile);
    env.extend(overrides);

    env
}

/// Replace the current process with claude.
///
/// This function never returns on success.
pub fn launch(profile: &Profile, extra_args: &[String]) -> Result<(), CcsError> {
    let resolved = resolve_profile_env(profile)?;
    let env = build_env(&resolved);
    let binary = find_claude_binary()?;

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&binary)
            .args(extra_args)
            .env_clear()
            .envs(&env)
            .exec();
        // exec() only returns on error
        Err(CcsError::Io(err))
    }

    #[cfg(not(unix))]
    {
        // Fallback for non-Unix: spawn and wait
        let status = std::process::Command::new(&binary)
            .args(extra_args)
            .env_clear()
            .envs(&env)
            .status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_claude_binary_via_env() {
        std::env::set_var("CLAUDE_BINARY", "/custom/path/to/claude");
        assert_eq!(find_claude_binary().unwrap(), "/custom/path/to/claude");
        std::env::remove_var("CLAUDE_BINARY");
    }

    #[test]
    fn test_build_env_includes_existing() {
        std::env::set_var("CCS_TEST_EXISTING", "keep-me");
        let env = build_env(&Profile::default());
        assert_eq!(
            env.get("CCS_TEST_EXISTING").map(|s| s.as_str()),
            Some("keep-me")
        );
        std::env::remove_var("CCS_TEST_EXISTING");
    }

    #[test]
    fn test_build_env_overlays_profile() {
        let profile = Profile {
            base_url: Some("http://localhost:11434/v1".into()),
            auth_token: Some("ollama".into()),
            ..Default::default()
        };
        let env = build_env(&profile);
        assert_eq!(env["ANTHROPIC_BASE_URL"], "http://localhost:11434/v1");
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "ollama");
    }

    #[test]
    fn test_build_env_empty_profile_preserves_existing() {
        let env = build_env(&Profile::default());
        // Should still have standard env vars
        assert!(env.contains_key("HOME") || env.contains_key("PATH"));
    }

    #[test]
    fn test_build_env_default_strips_shell_anthropic_vars() {
        // Simulate a user who has ANTHROPIC_BASE_URL set in their shell.
        std::env::set_var("ANTHROPIC_BASE_URL", "http://ollama.local/v1");
        std::env::set_var("ANTHROPIC_AUTH_TOKEN", "shell-token");

        // With default (empty) profile, those vars must NOT survive.
        let env = build_env(&Profile::default());
        assert!(
            !env.contains_key("ANTHROPIC_BASE_URL"),
            "default profile must strip shell ANTHROPIC_BASE_URL"
        );
        assert!(
            !env.contains_key("ANTHROPIC_AUTH_TOKEN"),
            "default profile must strip shell ANTHROPIC_AUTH_TOKEN"
        );

        std::env::remove_var("ANTHROPIC_BASE_URL");
        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    }

    #[test]
    fn test_build_env_profile_overrides_shell_vars() {
        // Shell has one base URL; profile specifies a different one.
        std::env::set_var("ANTHROPIC_BASE_URL", "http://shell.local/v1");

        let profile = Profile {
            base_url: Some("http://profile.local/v1".into()),
            ..Default::default()
        };
        let env = build_env(&profile);
        assert_eq!(env["ANTHROPIC_BASE_URL"], "http://profile.local/v1");

        std::env::remove_var("ANTHROPIC_BASE_URL");
    }
}

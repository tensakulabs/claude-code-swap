use std::collections::BTreeMap;

use crate::config::{build_env_overrides, resolve_profile_env, Profile};
use crate::error::CcsError;

/// Find the claude binary.
///
/// Search order:
///   1. CLAUDE_BINARY env var (for testing/overrides)
///   2. `which` lookup
///   3. Error with install hint
pub fn find_claude_binary() -> Result<String, CcsError> {
    if let Ok(override_path) = std::env::var("CLAUDE_BINARY") {
        return Ok(override_path);
    }
    match which::which("claude") {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(_) => Err(CcsError::BinaryNotFound),
    }
}

/// Build the full environment for exec.
///
/// Starts with current env, overlays ANTHROPIC_* vars from profile.
pub fn build_env(resolved_profile: &Profile) -> BTreeMap<String, String> {
    let mut env: BTreeMap<String, String> = std::env::vars().collect();
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
}

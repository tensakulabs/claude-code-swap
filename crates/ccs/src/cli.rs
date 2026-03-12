use std::io::IsTerminal;
use std::process;

use clap::{Parser, Subcommand};

use crate::color;
use crate::config::{config_file, load_config, save_config, warn_hardcoded_keys, Config, Profile};
use crate::doctor::run_doctor;
use crate::error::CcsError;
use crate::launcher::launch;
use crate::presets::PRESET_NAMES;
use crate::profiles::{add_profile, get_profile, list_profiles, remove_profile};
use crate::state::{get_active_profile, set_active_profile};
use crate::tester::test_profile;
use crate::wizard::run_wizard;
use crate::VERSION;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "ccs",
    about = "Claude Code provider profile switcher",
    version = VERSION,
    after_help = "\
Examples:
  ccs                         Launch with active profile
  ccs use ollama              Switch to ollama profile
  ccs run openrouter          One-shot launch with openrouter (does not change active)
  ccs status                  Show active profile
  ccs profile list            List all profiles
  ccs profile add mywork      Add a new profile interactively
  ccs profile capture mywork  Save current ANTHROPIC_* env vars as a profile
  ccs test                    Test active profile connectivity
  ccs doctor                  System health check
  ccs init                    First-time setup wizard"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Set active profile (persists across sessions)
    Use {
        /// Profile name
        profile: String,
    },

    /// One-shot launch with specific profile (does not change active)
    Run {
        /// Profile name
        profile: String,

        /// Additional args passed directly to claude
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        claude_args: Vec<String>,
    },

    /// Show active profile and its config
    Status,

    /// Manage profiles
    Profile {
        #[command(subcommand)]
        command: Option<ProfileCommand>,
    },

    /// Test provider connectivity
    Test {
        /// Profile name (default: active)
        profile: Option<String>,
    },

    /// Run system health checks
    Doctor,

    /// Interactive first-time setup wizard
    Init,

    /// Open config.yaml in $EDITOR
    Config,
}

#[derive(Subcommand)]
pub enum ProfileCommand {
    /// List all profiles
    List,

    /// Show full profile config
    Show {
        /// Profile name (default: active)
        name: Option<String>,
    },

    /// Add a new profile
    Add {
        /// Profile name
        name: String,

        /// Initialize from a provider preset
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(PRESET_NAMES))]
        preset: Option<String>,
    },

    /// Edit a profile in $EDITOR
    Edit {
        /// Profile name
        name: String,
    },

    /// Remove a profile
    Remove {
        /// Profile name
        name: String,
    },

    /// Create a profile from current ANTHROPIC_* environment variables
    Capture {
        /// Profile name to save as
        name: String,
    },
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

fn profile_summary(_name: &str, profile: &Profile) -> String {
    if profile == &Profile::default() {
        return "Anthropic subscription (Pro/Max)".to_string();
    }

    let base_url = profile.base_url.as_deref().unwrap_or("");
    let provider = if base_url.contains("ollama") || base_url.contains("11434") {
        "Local Ollama"
    } else if base_url.contains("openrouter") {
        "OpenRouter"
    } else if base_url.contains("googleapis") {
        "Google Gemini"
    } else if base_url.contains("openai.com") {
        "OpenAI"
    } else if !base_url.is_empty() {
        // Extract hostname
        base_url
            .split("//")
            .nth(1)
            .and_then(|s| s.split('/').next())
            .unwrap_or(base_url)
    } else {
        "Unknown"
    };

    let primary = profile
        .models
        .as_ref()
        .and_then(|m| m.sonnet.as_deref().or(m.haiku.as_deref()));
    match primary {
        Some(model) => format!("{provider} -> {model}"),
        None => provider.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

fn cmd_launch(extra_args: &[String]) -> Result<(), CcsError> {
    let config = load_config()?;
    warn_hardcoded_keys(&config);

    let active = get_active_profile();
    let profile = get_profile(&config, &active)?;

    launch(&profile, extra_args)?;
    Ok(())
}

fn cmd_use(name: &str) -> Result<(), CcsError> {
    let config = load_config()?;
    let profiles = &config.profiles;

    if name != "default" && !profiles.contains_key(name) {
        let available: Vec<String> = std::iter::once("default".to_string())
            .chain(profiles.keys().cloned())
            .collect();
        color::err_msg(&format!(
            "Profile '{}' not found. Available: {}",
            name,
            available.join(", ")
        ));
        process::exit(1);
    }

    set_active_profile(name)?;

    let profile = get_profile(&config, name).unwrap_or_default();
    let description = profile_summary(name, &profile);
    println!("Switched to: {name} ({description})");
    Ok(())
}

fn cmd_run(name: &str, claude_args: &[String]) -> Result<(), CcsError> {
    let config = load_config()?;
    warn_hardcoded_keys(&config);

    let profile = get_profile(&config, name)?;
    launch(&profile, claude_args)?;
    Ok(())
}

fn cmd_status() -> Result<(), CcsError> {
    println!("claude-swap v{VERSION}\n");

    let active = get_active_profile();
    let config = load_config()?;
    let profile = get_profile(&config, &active).unwrap_or_default();

    println!("Active profile: {active}");

    if profile == Profile::default() {
        println!("  (no configuration — Anthropic subscription)");
    } else {
        if let Some(ref base_url) = profile.base_url {
            println!("  Base URL: {base_url}");
        }
        if let Some(ref models) = profile.models {
            for (role, model) in &[
                ("Haiku", &models.haiku),
                ("Sonnet", &models.sonnet),
                ("Opus", &models.opus),
            ] {
                if let Some(m) = model {
                    println!("  {:<8} {m}", role);
                }
            }
        }
        if let Some(ref env_map) = profile.env {
            for (k, v) in env_map {
                println!("  {k}: {v}");
            }
        }
    }

    println!("\nRun \"ccs use <profile>\" to switch. Run \"ccs profile list\" for all profiles.");
    Ok(())
}

fn cmd_profile_list(config: &Config) {
    let active = get_active_profile();
    let all_names = list_profiles(config);

    for name in &all_names {
        let marker = if name == &active { "*" } else { " " };
        let profile = config.profiles.get(name).cloned().unwrap_or_default();
        let description = profile_summary(name, &profile);
        println!("  {marker} {:<20} {description}", name);
    }
}

fn cmd_profile_show(config: &Config, name: Option<&str>) -> Result<(), CcsError> {
    let name = name
        .map(|s| s.to_string())
        .unwrap_or_else(get_active_profile);
    let profile = get_profile(config, &name)?;

    println!("Profile: {name}");
    if profile == Profile::default() {
        println!("  (empty — Anthropic subscription, no overrides)");
    } else {
        let yaml = serde_yaml::to_string(&profile).unwrap_or_default();
        print!("{}", yaml.trim_end());
        println!();
    }
    Ok(())
}

fn cmd_profile_add(config: &Config, name: &str, preset: Option<&str>) -> Result<(), CcsError> {
    if name == "default" {
        color::err_msg("Cannot add a profile named 'default' — it already exists implicitly");
        process::exit(1);
    }

    if config.profiles.contains_key(name) {
        color::err_msg(&format!(
            "Profile '{name}' already exists. Use 'ccs profile edit {name}' to modify."
        ));
        process::exit(1);
    }

    if let Some(preset_name) = preset {
        let data = crate::presets::get_preset(preset_name);
        let new_config = add_profile(config, name, data);
        save_config(&new_config)?;
        println!("Profile \"{name}\" added from preset \"{preset_name}\".");
        println!("Edit it with: ccs profile edit {name}");
    } else {
        // Open editor with a template
        let template = Profile {
            base_url: Some(String::new()),
            auth_token: Some("${YOUR_API_KEY}".into()),
            api_key: Some(String::new()),
            models: Some(crate::config::Models {
                haiku: Some(String::new()),
                sonnet: Some(String::new()),
                opus: Some(String::new()),
            }),
            ..Default::default()
        };
        let yaml = format!(
            "# Profile: {name}\n# Edit and save to add.\n\n{}",
            serde_yaml::to_string(&template).unwrap_or_default()
        );

        let tmp = tempfile::Builder::new()
            .prefix(&format!("ccs-profile-{name}-"))
            .suffix(".yaml")
            .tempfile()
            .map_err(CcsError::Io)?;
        let tmp_path = tmp.path().to_path_buf();
        std::fs::write(&tmp_path, &yaml)?;

        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".into());
        let status = process::Command::new(&editor).arg(&tmp_path).status()?;
        if !status.success() {
            color::err_msg("Editor exited with error");
            process::exit(1);
        }

        let content = std::fs::read_to_string(&tmp_path)?;
        let profile_data: Profile = serde_yaml::from_str(&content)
            .map_err(|e| CcsError::Config(format!("Failed to parse profile: {e}")))?;

        let new_config = add_profile(config, name, profile_data);
        save_config(&new_config)?;
        println!("Profile \"{name}\" added.");
    }
    Ok(())
}

fn cmd_profile_edit(config: &Config, name: &str) -> Result<(), CcsError> {
    if name == "default" {
        color::err_msg("Cannot edit 'default' — it is the built-in Anthropic profile. Use 'ccs profile add <name>' to create a custom profile.");
        process::exit(1);
    }

    let profile = get_profile(config, name)?;

    let yaml = format!(
        "# Profile: {name}\n\n{}",
        serde_yaml::to_string(&profile).unwrap_or_default()
    );

    let tmp = tempfile::Builder::new()
        .prefix(&format!("ccs-profile-{name}-"))
        .suffix(".yaml")
        .tempfile()
        .map_err(CcsError::Io)?;
    let tmp_path = tmp.path().to_path_buf();
    std::fs::write(&tmp_path, &yaml)?;

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());

    loop {
        let status = process::Command::new(&editor).arg(&tmp_path).status()?;
        if !status.success() {
            color::err_msg("Editor exited with error");
            process::exit(1);
        }

        let content = std::fs::read_to_string(&tmp_path)?;
        match serde_yaml::from_str::<Profile>(&content) {
            Ok(new_data) => {
                let new_config = add_profile(config, name, new_data);
                save_config(&new_config)?;
                println!("Profile \"{name}\" updated.");
                return Ok(());
            }
            Err(e) => {
                println!("YAML error: {e}");
                if !std::io::stdin().is_terminal() {
                    process::exit(1);
                }
                let reopen = dialoguer::Confirm::new()
                    .with_prompt("Re-open in editor?")
                    .default(false)
                    .interact()
                    .unwrap_or(false);
                if !reopen {
                    process::exit(1);
                }
            }
        }
    }
}

fn cmd_profile_remove(config: &Config, name: &str) -> Result<(), CcsError> {
    let active = get_active_profile();
    if name == active {
        color::err_msg(&format!(
            "'{name}' is the active profile. Run 'ccs use default' first."
        ));
        process::exit(1);
    }

    if !config.profiles.contains_key(name) && name != "default" {
        color::err_msg(&format!("Profile '{name}' not found"));
        process::exit(1);
    }

    if std::io::stdin().is_terminal() {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!("Remove profile '{name}'?"))
            .default(false)
            .interact()
            .unwrap_or(false);
        if !confirm {
            println!("Aborted.");
            return Ok(());
        }
    }

    let new_config = remove_profile(config, name)?;
    save_config(&new_config)?;
    println!("Profile \"{name}\" removed.");
    Ok(())
}

fn cmd_test(profile_name: Option<&str>) -> Result<(), CcsError> {
    let config = load_config()?;
    let name = profile_name
        .map(|s| s.to_string())
        .unwrap_or_else(get_active_profile);
    let profile = get_profile(&config, &name)?;
    test_profile(&name, &profile);
    Ok(())
}

fn cmd_profile_capture(config: &Config, name: &str) -> Result<(), CcsError> {
    if name == "default" {
        color::err_msg("Cannot capture into 'default' — it is the built-in Anthropic profile");
        process::exit(1);
    }

    if config.profiles.contains_key(name) {
        color::err_msg(&format!(
            "Profile '{name}' already exists. Use 'ccs profile edit {name}' to modify."
        ));
        process::exit(1);
    }

    let Some(profile) = capture_env_profile() else {
        println!("No ANTHROPIC_* environment variables found in current shell.");
        println!("Nothing to capture. Set variables like ANTHROPIC_BASE_URL and try again.");
        return Ok(());
    };

    // Show what was captured
    println!("Captured environment variables:");
    if let Some(ref v) = profile.base_url {
        println!("  ANTHROPIC_BASE_URL        = {v}");
    }
    if let Some(ref v) = profile.auth_token {
        let masked = if v.len() > 8 {
            format!("{}…{}", &v[..4], &v[v.len() - 4..])
        } else {
            "***".to_string()
        };
        println!("  ANTHROPIC_AUTH_TOKEN      = {masked}");
    }
    if let Some(ref v) = profile.api_key {
        let masked = if v.len() > 8 {
            format!("{}…{}", &v[..4], &v[v.len() - 4..])
        } else {
            "***".to_string()
        };
        println!("  ANTHROPIC_API_KEY         = {masked}");
    }
    if let Some(ref m) = profile.models {
        if let Some(ref v) = m.haiku {
            println!("  ANTHROPIC_DEFAULT_HAIKU_MODEL  = {v}");
        }
        if let Some(ref v) = m.sonnet {
            println!("  ANTHROPIC_DEFAULT_SONNET_MODEL = {v}");
        }
        if let Some(ref v) = m.opus {
            println!("  ANTHROPIC_DEFAULT_OPUS_MODEL   = {v}");
        }
    }

    let new_config = add_profile(config, name, profile);
    save_config(&new_config)?;

    println!("\nProfile \"{name}\" saved.");
    println!("Switch back to Anthropic default with: ccs use default");
    println!("Switch to this profile with: ccs use {name}");
    Ok(())
}

fn cmd_config_open() -> Result<(), CcsError> {
    let path = config_file();
    if !path.exists() {
        println!(
            "Config file not found at {}. Run 'ccs init' first.",
            path.display()
        );
        return Ok(());
    }

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let status = process::Command::new(&editor).arg(&path).status()?;
    if !status.success() {
        color::err_msg("Editor exited with error");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// First-run auto-init
// ---------------------------------------------------------------------------

/// Read the current shell's ANTHROPIC_* env vars into a Profile.
/// Returns None if no overrides are set.
fn capture_env_profile() -> Option<Profile> {
    let base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
    let auth_token = std::env::var("ANTHROPIC_AUTH_TOKEN").ok();
    let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let haiku = std::env::var("ANTHROPIC_DEFAULT_HAIKU_MODEL").ok();
    let sonnet = std::env::var("ANTHROPIC_DEFAULT_SONNET_MODEL").ok();
    let opus = std::env::var("ANTHROPIC_DEFAULT_OPUS_MODEL").ok();

    let models = if haiku.is_some() || sonnet.is_some() || opus.is_some() {
        Some(crate::config::Models {
            haiku,
            sonnet,
            opus,
        })
    } else {
        None
    };

    let profile = Profile {
        base_url,
        auth_token,
        api_key,
        models,
        ..Default::default()
    };

    if profile == Profile::default() {
        None
    } else {
        Some(profile)
    }
}

/// Run silently on first use when no config exists.
///
/// Creates `~/.claude-code-swap/config.yaml` with a default profile.
/// If ANTHROPIC_* env vars are set, captures them as "captured" profile
/// and sets it as active so the user's existing setup keeps working.
fn auto_init() -> Result<(), CcsError> {
    let config_path = config_file();
    if config_path.exists() {
        return Ok(());
    }

    use crate::config::{ensure_config_dir, Config};
    use crate::profiles::add_profile;
    use crate::state::set_active_profile;
    use std::collections::BTreeMap;

    ensure_config_dir()?;

    let base_config = Config {
        profiles: BTreeMap::new(),
    };

    if let Some(captured) = capture_env_profile() {
        // User already has overrides — preserve them as "captured" profile
        let config = add_profile(&base_config, "captured", captured);
        save_config(&config)?;
        set_active_profile("captured")?;
        eprintln!(
            "ccs: initialized config at {} (your ANTHROPIC_* overrides saved as 'captured' profile)",
            config_path.display()
        );
        eprintln!("ccs: run 'ccs use default' to switch to Anthropic subscription.");
    } else {
        save_config(&base_config)?;
        eprintln!("ccs: initialized config at {}", config_path.display());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() -> Result<(), CcsError> {
    // First-run: silently create config if it doesn't exist yet.
    auto_init()?;

    // Try parsing with clap. If no subcommand, launch claude.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    // Let clap handle --help and --version normally
                    e.exit();
                }
                _ => {
                    // Unknown args — forward everything to claude as extra args
                    let args: Vec<String> = std::env::args().skip(1).collect();
                    return cmd_launch(&args);
                }
            }
        }
    };

    match cli.command {
        None => {
            // Bare `ccs` — launch with active profile, forward any extra args
            let args: Vec<String> = std::env::args().skip(1).collect();
            cmd_launch(&args)
        }
        Some(Command::Use { profile }) => cmd_use(&profile),
        Some(Command::Run {
            profile,
            claude_args,
        }) => cmd_run(&profile, &claude_args),
        Some(Command::Status) => cmd_status(),
        Some(Command::Profile { command }) => {
            let config = load_config()?;
            match command {
                None | Some(ProfileCommand::List) => {
                    cmd_profile_list(&config);
                    Ok(())
                }
                Some(ProfileCommand::Show { name }) => cmd_profile_show(&config, name.as_deref()),
                Some(ProfileCommand::Add { name, preset }) => {
                    cmd_profile_add(&config, &name, preset.as_deref())
                }
                Some(ProfileCommand::Edit { name }) => cmd_profile_edit(&config, &name),
                Some(ProfileCommand::Remove { name }) => cmd_profile_remove(&config, &name),
                Some(ProfileCommand::Capture { name }) => cmd_profile_capture(&config, &name),
            }
        }
        Some(Command::Test { profile }) => cmd_test(profile.as_deref()),
        Some(Command::Doctor) => {
            run_doctor();
            Ok(())
        }
        Some(Command::Init) => run_wizard(),
        Some(Command::Config) => cmd_config_open(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialise all tests that touch process-level env vars.
    /// Rust runs tests in parallel by default; env vars are process-global, so
    /// tests that mutate ANTHROPIC_* or CCS_CONFIG_DIR must not run concurrently.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ── capture_env_profile ────────────────────────────────────────────────

    #[test]
    fn test_capture_env_profile_empty_when_no_vars() {
        let _g = ENV_LOCK.lock().unwrap();
        // Ensure none of the managed vars are set.
        for key in &[
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ] {
            std::env::remove_var(key);
        }
        assert!(
            capture_env_profile().is_none(),
            "should return None when no ANTHROPIC_* vars are set"
        );
    }

    #[test]
    fn test_capture_env_profile_captures_base_url() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("ANTHROPIC_BASE_URL", "http://ollama.local/v1");
        // Clear others to avoid interference.
        for key in &[
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ] {
            std::env::remove_var(key);
        }

        let profile = capture_env_profile().expect("should capture when BASE_URL is set");
        assert_eq!(profile.base_url.as_deref(), Some("http://ollama.local/v1"));
        assert!(profile.auth_token.is_none());
        assert!(profile.models.is_none());

        std::env::remove_var("ANTHROPIC_BASE_URL");
    }

    #[test]
    fn test_capture_env_profile_captures_all_fields() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("ANTHROPIC_BASE_URL", "http://openrouter.ai/v1");
        std::env::set_var("ANTHROPIC_AUTH_TOKEN", "tok-abc");
        std::env::set_var("ANTHROPIC_API_KEY", "key-xyz");
        std::env::set_var("ANTHROPIC_DEFAULT_HAIKU_MODEL", "fast-model");
        std::env::set_var("ANTHROPIC_DEFAULT_SONNET_MODEL", "mid-model");
        std::env::set_var("ANTHROPIC_DEFAULT_OPUS_MODEL", "big-model");

        let profile = capture_env_profile().expect("should capture all fields");
        assert_eq!(profile.base_url.as_deref(), Some("http://openrouter.ai/v1"));
        assert_eq!(profile.auth_token.as_deref(), Some("tok-abc"));
        assert_eq!(profile.api_key.as_deref(), Some("key-xyz"));
        let models = profile.models.expect("models should be set");
        assert_eq!(models.haiku.as_deref(), Some("fast-model"));
        assert_eq!(models.sonnet.as_deref(), Some("mid-model"));
        assert_eq!(models.opus.as_deref(), Some("big-model"));

        for key in &[
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn test_capture_env_profile_model_only() {
        let _g = ENV_LOCK.lock().unwrap();
        for key in &[
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
        ] {
            std::env::remove_var(key);
        }
        std::env::set_var("ANTHROPIC_DEFAULT_SONNET_MODEL", "my-sonnet");
        std::env::remove_var("ANTHROPIC_DEFAULT_HAIKU_MODEL");
        std::env::remove_var("ANTHROPIC_DEFAULT_OPUS_MODEL");

        let profile = capture_env_profile().expect("model-only override should be captured");
        assert!(profile.base_url.is_none());
        let models = profile.models.expect("models should be set");
        assert_eq!(models.sonnet.as_deref(), Some("my-sonnet"));
        assert!(models.haiku.is_none());
        assert!(models.opus.is_none());

        std::env::remove_var("ANTHROPIC_DEFAULT_SONNET_MODEL");
    }

    // ── auto_init ─────────────────────────────────────────────────────────

    #[test]
    fn test_auto_init_creates_config_when_missing() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        // Clear ANTHROPIC vars so no captured profile is created.
        for key in &[
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ] {
            std::env::remove_var(key);
        }

        auto_init().unwrap();

        assert!(
            dir.path().join("config.yaml").exists(),
            "config.yaml should be created by auto_init"
        );
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_auto_init_skips_when_config_exists() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.yaml");
        std::fs::write(&config_path, "profiles: {}\n").unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());

        // Modify the file so we can detect if auto_init touched it.
        let before = std::fs::metadata(&config_path).unwrap().modified().unwrap();
        // Small sleep to ensure mtime would differ if written.
        std::thread::sleep(std::time::Duration::from_millis(10));
        auto_init().unwrap();
        let after = std::fs::metadata(&config_path).unwrap().modified().unwrap();

        assert_eq!(
            before, after,
            "auto_init must not overwrite existing config"
        );
        std::env::remove_var("CCS_CONFIG_DIR");
    }

    #[test]
    fn test_auto_init_captures_env_vars_as_captured_profile() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        std::env::set_var("ANTHROPIC_BASE_URL", "http://ollama.local/v1");
        for key in &[
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ] {
            std::env::remove_var(key);
        }

        auto_init().unwrap();

        let config = load_config().unwrap();
        assert!(
            config.profiles.contains_key("captured"),
            "auto_init should create 'captured' profile when ANTHROPIC_* vars are set"
        );
        assert_eq!(
            config.profiles["captured"].base_url.as_deref(),
            Some("http://ollama.local/v1")
        );

        std::env::remove_var("CCS_CONFIG_DIR");
        std::env::remove_var("ANTHROPIC_BASE_URL");
    }

    #[test]
    fn test_auto_init_no_captured_profile_without_env_vars() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CCS_CONFIG_DIR", dir.path());
        for key in &[
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ] {
            std::env::remove_var(key);
        }

        auto_init().unwrap();

        let config = load_config().unwrap();
        assert!(
            !config.profiles.contains_key("captured"),
            "auto_init must not create 'captured' profile when no ANTHROPIC_* vars are set"
        );

        std::env::remove_var("CCS_CONFIG_DIR");
    }
}

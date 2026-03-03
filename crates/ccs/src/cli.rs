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
// Entry point
// ---------------------------------------------------------------------------

pub fn run() -> Result<(), CcsError> {
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

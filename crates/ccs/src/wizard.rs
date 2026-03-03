use std::io::IsTerminal;

use dialoguer::{Confirm, Input, Select};

use crate::config::{config_file, ensure_config_dir, load_config, save_config, Models, Profile};
use crate::error::CcsError;
use crate::presets::{get_preset, PRESET_NAMES};
use crate::profiles::add_profile;
use crate::state::set_active_profile;

/// Interactive first-time setup wizard.
pub fn run_wizard() -> Result<(), CcsError> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        eprintln!("ccs: error: init requires an interactive terminal");
        return Ok(());
    }

    println!("Welcome to claude-swap!\n");

    let cfg_path = config_file();
    if cfg_path.exists() {
        let reinit = Confirm::new()
            .with_prompt("Config already exists. Reinitialize?")
            .default(false)
            .interact()
            .unwrap_or(false);
        if !reinit {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Check for claude binary
    match which::which("claude") {
        Ok(path) => println!("Found Claude Code at: {}\n", path.display()),
        Err(_) => println!("Warning: Claude Code not found in PATH.\n"),
    }

    let mut config = if cfg_path.exists() {
        load_config()?
    } else {
        crate::config::Config::default()
    };
    let mut profiles_added: Vec<String> = Vec::new();

    loop {
        let setup = Confirm::new()
            .with_prompt("Set up a provider profile?")
            .default(true)
            .interact()
            .unwrap_or(false);
        if !setup {
            break;
        }

        // Profile name
        let name: String = Input::new()
            .with_prompt("Profile name")
            .interact_text()
            .unwrap_or_default();
        let name = name.trim().to_string();
        if name.is_empty() || name == "default" {
            println!("Invalid profile name. Try again.");
            continue;
        }

        // Provider type
        let items: Vec<&str> = PRESET_NAMES.to_vec();
        println!("\nProvider types: {}", items.join(", "));
        let provider_idx = Select::new()
            .with_prompt("Provider type")
            .items(&items)
            .default(0)
            .interact()
            .unwrap_or(items.len() - 1); // default to custom on error
        let provider_type = items[provider_idx];

        let mut profile_data = get_preset(provider_type);

        match provider_type {
            "ollama" => {
                let base_url: String = Input::new()
                    .with_prompt("Base URL")
                    .default("http://localhost:11434/v1".into())
                    .interact_text()
                    .unwrap_or_default();
                profile_data.base_url = Some(base_url);

                let models = profile_data.models.clone().unwrap_or_default();
                let haiku: String = Input::new()
                    .with_prompt("Haiku model")
                    .default(models.haiku.unwrap_or_default())
                    .interact_text()
                    .unwrap_or_default();
                let sonnet: String = Input::new()
                    .with_prompt("Sonnet model")
                    .default(models.sonnet.unwrap_or_default())
                    .interact_text()
                    .unwrap_or_default();
                let opus: String = Input::new()
                    .with_prompt("Opus model")
                    .default(models.opus.unwrap_or_default())
                    .interact_text()
                    .unwrap_or_default();
                profile_data.models = Some(Models {
                    haiku: if haiku.is_empty() { None } else { Some(haiku) },
                    sonnet: if sonnet.is_empty() {
                        None
                    } else {
                        Some(sonnet)
                    },
                    opus: if opus.is_empty() { None } else { Some(opus) },
                });
            }
            "openrouter" | "gemini" | "openai" => {
                let env_var_hint = match provider_type {
                    "openrouter" => "OPENROUTER_API_KEY",
                    "gemini" => "GEMINI_API_KEY",
                    "openai" => "OPENAI_API_KEY",
                    _ => "API_KEY",
                };
                println!(
                    "API key reference (use ${{ENV_VAR}} or press Enter for ${{{}}})",
                    env_var_hint
                );
                let key_input: String = Input::new()
                    .with_prompt("API key env var")
                    .default(format!("${{{env_var_hint}}}"))
                    .interact_text()
                    .unwrap_or_default();
                profile_data.auth_token = Some(key_input);
            }
            "custom" => {
                let base_url: String = Input::new()
                    .with_prompt("Base URL")
                    .interact_text()
                    .unwrap_or_default();
                profile_data.base_url = Some(base_url);

                println!("Auth token (use ${{ENV_VAR}} for env var references)");
                let auth: String = Input::new()
                    .with_prompt("Auth token")
                    .interact_text()
                    .unwrap_or_default();
                profile_data.auth_token = Some(auth);

                let has_models = Confirm::new()
                    .with_prompt("Configure model overrides?")
                    .default(false)
                    .interact()
                    .unwrap_or(false);
                if has_models {
                    let h: String = Input::new()
                        .with_prompt("Haiku model (or Enter to skip)")
                        .default(String::new())
                        .interact_text()
                        .unwrap_or_default();
                    let s: String = Input::new()
                        .with_prompt("Sonnet model (or Enter to skip)")
                        .default(String::new())
                        .interact_text()
                        .unwrap_or_default();
                    let o: String = Input::new()
                        .with_prompt("Opus model (or Enter to skip)")
                        .default(String::new())
                        .interact_text()
                        .unwrap_or_default();
                    let haiku = if h.is_empty() { None } else { Some(h) };
                    let sonnet = if s.is_empty() { None } else { Some(s) };
                    let opus = if o.is_empty() { None } else { Some(o) };
                    if haiku.is_some() || sonnet.is_some() || opus.is_some() {
                        profile_data.models = Some(Models {
                            haiku,
                            sonnet,
                            opus,
                        });
                    }
                }
            }
            _ => {}
        }

        config = add_profile(&config, &name, profile_data);
        profiles_added.push(name.clone());
        println!("\nProfile \"{name}\" saved.");

        let another = Confirm::new()
            .with_prompt("Add another profile?")
            .default(false)
            .interact()
            .unwrap_or(false);
        if !another {
            break;
        }
    }

    if profiles_added.is_empty() && !cfg_path.exists() {
        config = crate::config::Config {
            profiles: std::collections::BTreeMap::from([("default".into(), Profile::default())]),
        };
    }

    // Ask for active profile
    let all_profiles: Vec<String> = std::iter::once("default".to_string())
        .chain(profiles_added.iter().cloned())
        .collect();

    let active = if !profiles_added.is_empty() {
        println!("\nAvailable profiles: {}", all_profiles.join(", "));
        let default_active = profiles_added
            .first()
            .cloned()
            .unwrap_or_else(|| "default".into());
        let input: String = Input::new()
            .with_prompt("Set active profile")
            .default(default_active)
            .interact_text()
            .unwrap_or_else(|_| "default".into());
        if all_profiles.contains(&input) {
            input
        } else {
            "default".to_string()
        }
    } else {
        "default".to_string()
    };

    ensure_config_dir()?;
    save_config(&config)?;
    set_active_profile(&active)?;

    println!("\nSetup complete! Active profile: {active}");
    println!("\nRun \"ccs\" to launch Claude Code.");
    if !profiles_added.is_empty() {
        println!("Run \"ccs use <profile>\" to switch providers.");
    }

    Ok(())
}

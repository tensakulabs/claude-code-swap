use std::io::IsTerminal;

use dialoguer::{Confirm, Input, Select};

use crate::config::{config_file, ensure_config_dir, load_config, save_config, Models, Profile};
use crate::error::CcsError;
use crate::launcher::find_claude_binary;
use crate::presets::{
    get_model_suggestions, get_preset, get_token_hint, TierSuggestions, PRESET_NAMES,
};
use crate::profiles::add_profile;
use crate::state::set_active_profile;

/// Free-text model prompt with a pre-filled default.
fn prompt_model_free(label: &str, default: &str) -> String {
    Input::new()
        .with_prompt(label)
        .default(default.to_string())
        .interact_text()
        .unwrap_or_default()
}

/// Select a model from a suggestion list.
/// Shows suggestions first, then a "custom…" option for free-text entry.
/// Falls back to the preset default if the current default isn't in the list.
fn prompt_model_select(label: &str, tier: &TierSuggestions, current: Option<&str>) -> String {
    const CUSTOM_OPT: &str = "custom…";

    let mut items: Vec<&str> = tier.models.to_vec();
    items.push(CUSTOM_OPT);

    // Try to pre-select the current default in the list.
    let default_idx = current
        .and_then(|cur| items.iter().position(|&m| m == cur))
        .unwrap_or(0);

    let idx = Select::new()
        .with_prompt(label)
        .items(&items)
        .default(default_idx)
        .interact()
        .unwrap_or(0);

    if items[idx] == CUSTOM_OPT {
        prompt_model_free(
            &format!("{label} (enter model name)"),
            current.unwrap_or(""),
        )
    } else {
        items[idx].to_string()
    }
}

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
    match find_claude_binary() {
        Ok(path) => println!("Found Claude Code at: {path}\n"),
        Err(_) => println!("Warning: Claude Code not found in PATH or common install locations.\n"),
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
                let haiku = prompt_model_free("Haiku model", models.haiku.as_deref().unwrap_or(""));
                let sonnet =
                    prompt_model_free("Sonnet model", models.sonnet.as_deref().unwrap_or(""));
                let opus = prompt_model_free("Opus model", models.opus.as_deref().unwrap_or(""));
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
                    let h = prompt_model_free("Haiku model (or Enter to skip)", "");
                    let s = prompt_model_free("Sonnet model (or Enter to skip)", "");
                    let o = prompt_model_free("Opus model (or Enter to skip)", "");
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
            _ => {
                // API-key providers: prompt for the key, then optionally let the user
                // pick models from the suggestion list.
                let default_token = profile_data
                    .auth_token
                    .clone()
                    .unwrap_or_else(|| "${API_KEY}".into());
                if let Some((env_var, url)) = get_token_hint(provider_type) {
                    println!("  Get your API key: {url}");
                    println!("  Set it as: export {env_var}=<your-key>");
                    println!("  Or enter the key directly below (not recommended).");
                }
                let key_input: String = Input::new()
                    .with_prompt("Auth token (env var ref or raw key)")
                    .default(default_token)
                    .interact_text()
                    .unwrap_or_default();
                profile_data.auth_token = Some(key_input);

                if let Some(suggestions) = get_model_suggestions(provider_type) {
                    if suggestions.has_choices() {
                        let customize = Confirm::new()
                            .with_prompt("Customize models? (Enter to keep defaults)")
                            .default(false)
                            .interact()
                            .unwrap_or(false);
                        if customize {
                            let models = profile_data.models.clone().unwrap_or_default();
                            let haiku = prompt_model_select(
                                "Haiku (fast) model",
                                &suggestions.haiku,
                                models.haiku.as_deref(),
                            );
                            let sonnet = prompt_model_select(
                                "Sonnet (mid) model",
                                &suggestions.sonnet,
                                models.sonnet.as_deref(),
                            );
                            let opus = prompt_model_select(
                                "Opus (best) model",
                                &suggestions.opus,
                                models.opus.as_deref(),
                            );
                            profile_data.models = Some(Models {
                                haiku: Some(haiku),
                                sonnet: Some(sonnet),
                                opus: Some(opus),
                            });
                        }
                    }
                }
            }
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

use std::fs;

use regex::Regex;

use crate::color;
use crate::config::{config_dir, config_file, load_config};
use crate::launcher::find_claude_binary;
use crate::state::get_active_profile;
use crate::VERSION;

#[derive(Debug)]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
}

pub fn check_claude_binary() -> CheckResult {
    match find_claude_binary() {
        Ok(path) => CheckResult {
            name: "Claude Code".into(),
            status: CheckStatus::Ok,
            message: path,
        },
        Err(_) => CheckResult {
            name: "Claude Code".into(),
            status: CheckStatus::Fail,
            message: "Not found in PATH. Install: npm install -g @anthropic-ai/claude-code".into(),
        },
    }
}

pub fn check_config_exists() -> CheckResult {
    let path = config_file();
    if path.exists() {
        CheckResult {
            name: "config.yaml".into(),
            status: CheckStatus::Ok,
            message: path.display().to_string(),
        }
    } else {
        CheckResult {
            name: "config.yaml".into(),
            status: CheckStatus::Warn,
            message: format!("Not found at {}. Run 'ccs init'.", path.display()),
        }
    }
}

pub fn check_config_parseable() -> CheckResult {
    let path = config_file();
    if !path.exists() {
        return CheckResult {
            name: "config.yaml (parse)".into(),
            status: CheckStatus::Warn,
            message: "File not found — skipping parse check".into(),
        };
    }
    match load_config() {
        Ok(config) => {
            let count = config.profiles.len();
            CheckResult {
                name: "config.yaml (parse)".into(),
                status: CheckStatus::Ok,
                message: format!("Valid YAML, {count} profile(s)"),
            }
        }
        Err(e) => CheckResult {
            name: "config.yaml (parse)".into(),
            status: CheckStatus::Fail,
            message: format!("Invalid YAML: {e}"),
        },
    }
}

pub fn check_config_dir_permissions() -> CheckResult {
    let dir = config_dir();
    if !dir.exists() {
        return CheckResult {
            name: "~/.claude-code-swap/ permissions".into(),
            status: CheckStatus::Warn,
            message: "Directory not found".into(),
        };
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match fs::metadata(&dir) {
            Ok(meta) => {
                let mode = meta.permissions().mode() & 0o777;
                if mode == 0o700 {
                    CheckResult {
                        name: "~/.claude-code-swap/ permissions".into(),
                        status: CheckStatus::Ok,
                        message: "0700".into(),
                    }
                } else {
                    CheckResult {
                        name: "~/.claude-code-swap/ permissions".into(),
                        status: CheckStatus::Warn,
                        message: format!(
                            "Expected 0700, got {:#o}. Fix: chmod 700 ~/.claude-code-swap/",
                            mode
                        ),
                    }
                }
            }
            Err(e) => CheckResult {
                name: "~/.claude-code-swap/ permissions".into(),
                status: CheckStatus::Warn,
                message: format!("Cannot read permissions: {e}"),
            },
        }
    }

    #[cfg(not(unix))]
    {
        CheckResult {
            name: "~/.claude-code-swap/ permissions".into(),
            status: CheckStatus::Ok,
            message: "(permission check skipped on non-Unix)".into(),
        }
    }
}

pub fn check_active_profile_valid() -> CheckResult {
    let active = get_active_profile();
    let path = config_file();

    if !path.exists() {
        if active == "default" {
            return CheckResult {
                name: "Active profile".into(),
                status: CheckStatus::Ok,
                message: "default (no config)".into(),
            };
        }
        return CheckResult {
            name: "Active profile".into(),
            status: CheckStatus::Warn,
            message: format!("'{active}' (no config file)"),
        };
    }

    match load_config() {
        Ok(config) => {
            if active == "default" || config.profiles.contains_key(&active) {
                CheckResult {
                    name: "Active profile".into(),
                    status: CheckStatus::Ok,
                    message: active,
                }
            } else {
                CheckResult {
                    name: "Active profile".into(),
                    status: CheckStatus::Fail,
                    message: format!("'{active}' not found in config. Run 'ccs use default'."),
                }
            }
        }
        Err(_) => CheckResult {
            name: "Active profile".into(),
            status: CheckStatus::Warn,
            message: format!("Could not verify '{active}'"),
        },
    }
}

pub fn check_env_refs_resolvable() -> CheckResult {
    let path = config_file();
    if !path.exists() {
        return CheckResult {
            name: "Env var refs (active)".into(),
            status: CheckStatus::Warn,
            message: "No config — skipping".into(),
        };
    }

    let active = get_active_profile();
    if active == "default" {
        return CheckResult {
            name: "Env var refs (active)".into(),
            status: CheckStatus::Ok,
            message: "default profile (no refs)".into(),
        };
    }

    let config = match load_config() {
        Ok(c) => c,
        Err(_) => {
            return CheckResult {
                name: "Env var refs (active)".into(),
                status: CheckStatus::Warn,
                message: "Could not load config".into(),
            }
        }
    };

    let profile = match config.profiles.get(&active) {
        Some(p) => p,
        None => {
            return CheckResult {
                name: "Env var refs (active)".into(),
                status: CheckStatus::Warn,
                message: format!("Profile '{active}' not found"),
            }
        }
    };

    // Scan all string values for ${VAR} references
    let env_ref_pattern = Regex::new(r"\$\{([^}]+)\}").unwrap();
    let yaml_value = serde_yaml::to_value(profile).unwrap_or_default();
    let mut total_refs = 0;
    let mut missing: Vec<String> = Vec::new();
    find_env_refs(&env_ref_pattern, &yaml_value, &mut total_refs, &mut missing);

    if missing.is_empty() {
        CheckResult {
            name: "Env var refs (active)".into(),
            status: CheckStatus::Ok,
            message: format!("All {total_refs} ref(s) resolved"),
        }
    } else {
        let missing_str = missing
            .iter()
            .map(|v| format!("${{{v}}}"))
            .collect::<Vec<_>>()
            .join(", ");
        CheckResult {
            name: "Env var refs (active)".into(),
            status: CheckStatus::Fail,
            message: format!("Unset variable(s) in '{active}': {missing_str}"),
        }
    }
}

fn find_env_refs(
    pattern: &Regex,
    value: &serde_yaml::Value,
    total: &mut usize,
    missing: &mut Vec<String>,
) {
    match value {
        serde_yaml::Value::String(s) => {
            for cap in pattern.captures_iter(s) {
                *total += 1;
                let var = &cap[1];
                if std::env::var(var).is_err() {
                    missing.push(var.to_string());
                }
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (_, v) in map {
                find_env_refs(pattern, v, total, missing);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for v in seq {
                find_env_refs(pattern, v, total, missing);
            }
        }
        _ => {}
    }
}

/// Run all health checks and print results.
pub fn run_doctor() {
    println!("claude-swap v{VERSION}\n");

    let checks = vec![
        check_claude_binary(),
        check_config_exists(),
        check_config_parseable(),
        check_config_dir_permissions(),
        check_active_profile_valid(),
        check_env_refs_resolvable(),
    ];

    for check in checks {
        let (icon, colorize): (&str, fn(&str) -> String) = match check.status {
            CheckStatus::Ok => ("[OK]  ", color::green as fn(&str) -> String),
            CheckStatus::Warn => ("[WARN]", color::yellow as fn(&str) -> String),
            CheckStatus::Fail => ("[FAIL]", color::red as fn(&str) -> String),
        };
        println!("{} {}: {}", colorize(icon), check.name, check.message);
    }
}

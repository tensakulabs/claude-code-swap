use crate::config::{Models, Profile};

/// Per-tier model suggestions for a preset.
/// The first item in each list is the preset default.
pub struct TierSuggestions {
    pub models: &'static [&'static str],
}

pub struct ModelSuggestions {
    pub haiku: TierSuggestions,
    pub sonnet: TierSuggestions,
    pub opus: TierSuggestions,
}

impl ModelSuggestions {
    /// Returns true if any tier has more than one suggestion (i.e. worth showing a picker).
    pub fn has_choices(&self) -> bool {
        self.haiku.models.len() > 1
            || self.sonnet.models.len() > 1
            || self.opus.models.len() > 1
    }
}

pub const PRESET_NAMES: &[&str] = &[
    "ollama",
    "openrouter",
    "gemini",
    "openai",
    "minimax",
    "kimi",
    "zai",
    "alibaba",
    "custom",
];

/// Return a preset profile stripped of internal metadata.
pub fn get_preset(name: &str) -> Profile {
    match name {
        "ollama" => Profile {
            base_url: Some("http://localhost:11434/v1".into()),
            auth_token: Some("ollama".into()),
            models: Some(Models {
                haiku: Some("llama3.2:3b".into()),
                sonnet: Some("llama3.1:8b".into()),
                opus: Some("llama3.1:70b".into()),
            }),
            ..Default::default()
        },
        "openrouter" => Profile {
            base_url: Some("https://openrouter.ai/api/v1".into()),
            auth_token: Some("${OPENROUTER_API_KEY}".into()),
            api_key: Some(String::new()),
            models: Some(Models {
                haiku: Some("qwen/qwen3-4b:free".into()),
                sonnet: Some("qwen/qwen3-coder:free".into()),
                opus: Some("deepseek/deepseek-r1-0528:free".into()),
            }),
            ..Default::default()
        },
        "gemini" => Profile {
            base_url: Some("https://generativelanguage.googleapis.com/v1beta/openai".into()),
            auth_token: Some("${GEMINI_API_KEY}".into()),
            api_key: Some(String::new()),
            models: Some(Models {
                haiku: Some("gemini-2.0-flash".into()),
                sonnet: Some("gemini-2.5-flash".into()),
                opus: Some("gemini-2.5-pro".into()),
            }),
            ..Default::default()
        },
        "openai" => Profile {
            base_url: Some("https://api.openai.com/v1".into()),
            auth_token: Some("${OPENAI_API_KEY}".into()),
            api_key: Some(String::new()),
            models: Some(Models {
                haiku: Some("gpt-4o-mini".into()),
                sonnet: Some("gpt-4.1".into()),
                opus: Some("o3".into()),
            }),
            ..Default::default()
        },
        // MiniMax — Anthropic-compatible endpoint
        // Set MINIMAX_API_KEY before use.
        // API_TIMEOUT_MS required: MiniMax models are slow and will time out without it.
        "minimax" => Profile {
            base_url: Some("https://api.minimax.io/anthropic".into()),
            auth_token: Some("${MINIMAX_API_KEY}".into()),
            models: Some(Models {
                haiku: Some("MiniMax-M2.5".into()),
                sonnet: Some("MiniMax-M2.5".into()),
                opus: Some("MiniMax-M2.5".into()),
            }),
            env: Some({
                let mut m = std::collections::BTreeMap::new();
                m.insert("ANTHROPIC_MODEL".into(), "MiniMax-M2.5".into());
                m.insert("ANTHROPIC_SMALL_FAST_MODEL".into(), "MiniMax-M2.5".into());
                m.insert("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(), "1".into());
                m.insert("API_TIMEOUT_MS".into(), "3000000".into());
                m
            }),
            ..Default::default()
        },
        // Kimi K2.5 (Moonshot AI) — Anthropic-compatible endpoint
        // Set MOONSHOT_API_KEY before use.
        // API_TIMEOUT_MS required: Kimi models are slow and will time out without it.
        "kimi" => Profile {
            base_url: Some("https://api.moonshot.ai/anthropic".into()),
            auth_token: Some("${MOONSHOT_API_KEY}".into()),
            models: Some(Models {
                haiku: Some("kimi-k2.5".into()),
                sonnet: Some("kimi-k2.5".into()),
                opus: Some("kimi-k2.5".into()),
            }),
            env: Some({
                let mut m = std::collections::BTreeMap::new();
                m.insert("ANTHROPIC_MODEL".into(), "kimi-k2.5".into());
                m.insert("ANTHROPIC_SMALL_FAST_MODEL".into(), "kimi-k2.5".into());
                m.insert("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(), "1".into());
                m.insert("API_TIMEOUT_MS".into(), "600000".into());
                m
            }),
            ..Default::default()
        },
        // Z.ai (Zhipu GLM) — Anthropic-compatible endpoint
        // Set ZAI_API_KEY before use.
        // API_TIMEOUT_MS required: GLM models are slow and will time out without it.
        "zai" => Profile {
            base_url: Some("https://api.z.ai/api/anthropic".into()),
            auth_token: Some("${ZAI_API_KEY}".into()),
            models: Some(Models {
                haiku: Some("GLM-4.5-Air".into()),
                sonnet: Some("GLM-4.7".into()),
                opus: Some("GLM-4.7".into()),
            }),
            env: Some({
                let mut m = std::collections::BTreeMap::new();
                m.insert("API_TIMEOUT_MS".into(), "3000000".into());
                m
            }),
            ..Default::default()
        },
        // Alibaba Cloud Coding Plan — Anthropic-compatible aggregator endpoint
        // Hosts models from multiple providers (Qwen, Kimi, GLM, MiniMax) under one URL.
        // Set ALIBABA_API_KEY to your Coding Plan API key (sk-sp-xxxxx format).
        // Note: Coding Plan API key is different from the standard DashScope pay-as-you-go key.
        "alibaba" => Profile {
            base_url: Some("https://coding-intl.dashscope.aliyuncs.com/apps/anthropic".into()),
            auth_token: Some("${ALIBABA_API_KEY}".into()),
            models: Some(Models {
                haiku: Some("qwen3.5-plus".into()),
                sonnet: Some("kimi-k2.5".into()),
                opus: Some("glm-5".into()),
            }),
            env: Some({
                let mut m = std::collections::BTreeMap::new();
                m.insert("ANTHROPIC_MODEL".into(), "kimi-k2.5".into());
                m.insert("ANTHROPIC_SMALL_FAST_MODEL".into(), "qwen3.5-plus".into());
                m
            }),
            ..Default::default()
        },
        _ => Profile::default(),
    }
}

/// Return the human-readable description for a preset.
pub fn get_preset_description(name: &str) -> &'static str {
    match name {
        "ollama" => "Local Ollama instance (no API key needed)",
        "openrouter" => "OpenRouter (set OPENROUTER_API_KEY)",
        "gemini" => "Google Gemini via OpenAI-compat endpoint (set GEMINI_API_KEY)",
        "openai" => "OpenAI direct (set OPENAI_API_KEY)",
        "minimax" => "MiniMax M2.5 via Anthropic-compat endpoint (set MINIMAX_API_KEY)",
        "kimi" => "Kimi K2.5 / Moonshot AI via Anthropic-compat endpoint (set MOONSHOT_API_KEY)",
        "zai" => "Z.ai / Zhipu GLM (set ZAI_API_KEY)",
        "alibaba" => "Alibaba Cloud Qwen via Anthropic-compat endpoint (set ALIBABA_API_KEY)",
        "custom" => "Custom provider — fill in manually",
        _ => "Unknown provider",
    }
}

/// Return a hint shown to the user explaining where to get their API token.
/// Returns (env_var_name, instructions_url_or_note).
pub fn get_token_hint(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "openrouter" => Some(("OPENROUTER_API_KEY", "https://openrouter.ai/settings/keys")),
        "gemini" => Some(("GEMINI_API_KEY", "https://aistudio.google.com/apikey")),
        "openai" => Some(("OPENAI_API_KEY", "https://platform.openai.com/api-keys — already use Codex CLI or other OpenAI tools? Your OPENAI_API_KEY is already set, just press Enter")),
        "minimax" => Some(("MINIMAX_API_KEY", "https://platform.minimax.io/user-center/basic-information/interface-key")),
        "kimi" => Some(("MOONSHOT_API_KEY", "https://platform.moonshot.ai/console/api-keys")),
        "zai" => Some(("ZAI_API_KEY", "https://openplatform.z.ai/api-keys")),
        "alibaba" => Some(("ALIBABA_API_KEY", "https://bailian.console.aliyun.com/ — use the Coding Plan API key (sk-sp-xxxxx)")),
        _ => None,
    }
}

/// Return alternative model suggestions for each tier, or None if the provider
/// has only one model or suggestions are not meaningful (e.g. ollama, custom).
/// The first item in each list is the preset default.
pub fn get_model_suggestions(name: &str) -> Option<ModelSuggestions> {
    match name {
        "openrouter" => Some(ModelSuggestions {
            haiku: TierSuggestions {
                models: &[
                    "qwen/qwen3-4b:free",
                    "google/gemini-2.0-flash-exp:free",
                    "meta-llama/llama-3.2-3b-instruct:free",
                ],
            },
            sonnet: TierSuggestions {
                models: &[
                    "qwen/qwen3-coder:free",
                    "google/gemini-2.5-flash-preview:free",
                    "meta-llama/llama-3.3-70b-instruct:free",
                ],
            },
            opus: TierSuggestions {
                models: &[
                    "deepseek/deepseek-r1-0528:free",
                    "qwen/qwen3-235b-a22b:free",
                    "google/gemini-2.5-pro-exp-03-25:free",
                ],
            },
        }),
        "gemini" => Some(ModelSuggestions {
            haiku: TierSuggestions {
                models: &["gemini-2.0-flash", "gemini-2.0-flash-lite"],
            },
            sonnet: TierSuggestions {
                models: &["gemini-2.5-flash", "gemini-2.0-flash"],
            },
            opus: TierSuggestions {
                models: &["gemini-2.5-pro", "gemini-2.5-flash"],
            },
        }),
        "openai" => Some(ModelSuggestions {
            haiku: TierSuggestions {
                models: &["gpt-4o-mini", "o4-mini"],
            },
            sonnet: TierSuggestions {
                models: &["gpt-4.1", "gpt-4o"],
            },
            opus: TierSuggestions {
                models: &["o3", "o4", "gpt-4.1"],
            },
        }),
        "zai" => Some(ModelSuggestions {
            haiku: TierSuggestions {
                models: &["GLM-4.5-Air"],
            },
            sonnet: TierSuggestions {
                models: &["GLM-4.7"],
            },
            opus: TierSuggestions {
                models: &["GLM-4.7", "glm-5"],
            },
        }),
        "alibaba" => Some(ModelSuggestions {
            haiku: TierSuggestions {
                models: &["qwen3.5-plus", "MiniMax-M2.5"],
            },
            sonnet: TierSuggestions {
                models: &[
                    "kimi-k2.5",
                    "qwen3-coder-next",
                    "qwen3-coder-plus",
                    "MiniMax-M2.5",
                ],
            },
            opus: TierSuggestions {
                models: &["glm-5", "kimi-k2.5", "qwen3-coder-next"],
            },
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn models(p: &Profile) -> &Models {
        p.models.as_ref().expect("preset must have models")
    }

    fn env_val<'a>(p: &'a Profile, key: &str) -> Option<&'a str> {
        p.env.as_ref()?.get(key).map(|s| s.as_str())
    }

    // ── generic invariants ───────────────────────────────────────────────────

    #[test]
    fn test_preset_names_all_exist() {
        for name in PRESET_NAMES {
            let preset = get_preset(name);
            if *name != "custom" {
                assert!(preset.base_url.is_some(), "preset '{name}' missing base_url");
            }
        }
    }

    #[test]
    fn test_all_non_custom_presets_have_models() {
        for name in PRESET_NAMES {
            if *name == "custom" {
                continue;
            }
            let p = get_preset(name);
            let m = models(&p);
            assert!(m.haiku.is_some(), "preset '{name}' missing haiku model");
            assert!(m.sonnet.is_some(), "preset '{name}' missing sonnet model");
            assert!(m.opus.is_some(), "preset '{name}' missing opus model");
        }
    }

    #[test]
    fn test_all_non_custom_presets_have_auth_token() {
        for name in PRESET_NAMES {
            if *name == "custom" {
                continue;
            }
            let p = get_preset(name);
            assert!(p.auth_token.is_some(), "preset '{name}' missing auth_token");
        }
    }

    #[test]
    fn test_custom_preset_is_empty() {
        let p = get_preset("custom");
        assert!(p.base_url.is_none());
        assert!(p.auth_token.is_none());
    }

    // ── per-provider pin tests ───────────────────────────────────────────────

    #[test]
    fn test_ollama_preset() {
        let p = get_preset("ollama");
        assert_eq!(p.base_url.as_deref(), Some("http://localhost:11434/v1"));
        assert_eq!(p.auth_token.as_deref(), Some("ollama"));
        assert_eq!(models(&p).haiku.as_deref(), Some("llama3.2:3b"));
        assert_eq!(models(&p).sonnet.as_deref(), Some("llama3.1:8b"));
        assert_eq!(models(&p).opus.as_deref(), Some("llama3.1:70b"));
    }

    #[test]
    fn test_openrouter_preset() {
        let p = get_preset("openrouter");
        assert_eq!(p.base_url.as_deref(), Some("https://openrouter.ai/api/v1"));
        assert_eq!(p.auth_token.as_deref(), Some("${OPENROUTER_API_KEY}"));
        assert_eq!(models(&p).haiku.as_deref(), Some("qwen/qwen3-4b:free"));
        assert_eq!(models(&p).sonnet.as_deref(), Some("qwen/qwen3-coder:free"));
        assert_eq!(models(&p).opus.as_deref(), Some("deepseek/deepseek-r1-0528:free"));
    }

    #[test]
    fn test_gemini_preset() {
        let p = get_preset("gemini");
        assert_eq!(
            p.base_url.as_deref(),
            Some("https://generativelanguage.googleapis.com/v1beta/openai")
        );
        assert_eq!(p.auth_token.as_deref(), Some("${GEMINI_API_KEY}"));
        assert_eq!(models(&p).haiku.as_deref(), Some("gemini-2.0-flash"));
        assert_eq!(models(&p).sonnet.as_deref(), Some("gemini-2.5-flash"));
        assert_eq!(models(&p).opus.as_deref(), Some("gemini-2.5-pro"));
    }

    #[test]
    fn test_openai_preset() {
        let p = get_preset("openai");
        assert_eq!(p.base_url.as_deref(), Some("https://api.openai.com/v1"));
        assert_eq!(p.auth_token.as_deref(), Some("${OPENAI_API_KEY}"));
        assert_eq!(models(&p).haiku.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(models(&p).sonnet.as_deref(), Some("gpt-4.1"));
        assert_eq!(models(&p).opus.as_deref(), Some("o3"));
    }

    #[test]
    fn test_minimax_preset() {
        let p = get_preset("minimax");
        assert_eq!(p.base_url.as_deref(), Some("https://api.minimax.io/anthropic"));
        assert_eq!(p.auth_token.as_deref(), Some("${MINIMAX_API_KEY}"));
        assert_eq!(models(&p).haiku.as_deref(), Some("MiniMax-M2.5"));
        assert_eq!(models(&p).sonnet.as_deref(), Some("MiniMax-M2.5"));
        assert_eq!(models(&p).opus.as_deref(), Some("MiniMax-M2.5"));
        assert_eq!(env_val(&p, "ANTHROPIC_MODEL"), Some("MiniMax-M2.5"));
        assert_eq!(env_val(&p, "ANTHROPIC_SMALL_FAST_MODEL"), Some("MiniMax-M2.5"));
        assert_eq!(env_val(&p, "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"), Some("1"));
        assert_eq!(env_val(&p, "API_TIMEOUT_MS"), Some("3000000"));
    }

    #[test]
    fn test_kimi_preset() {
        let p = get_preset("kimi");
        assert_eq!(p.base_url.as_deref(), Some("https://api.moonshot.ai/anthropic"));
        assert_eq!(p.auth_token.as_deref(), Some("${MOONSHOT_API_KEY}"));
        assert_eq!(models(&p).haiku.as_deref(), Some("kimi-k2.5"));
        assert_eq!(models(&p).sonnet.as_deref(), Some("kimi-k2.5"));
        assert_eq!(models(&p).opus.as_deref(), Some("kimi-k2.5"));
        assert_eq!(env_val(&p, "ANTHROPIC_MODEL"), Some("kimi-k2.5"));
        assert_eq!(env_val(&p, "ANTHROPIC_SMALL_FAST_MODEL"), Some("kimi-k2.5"));
        assert_eq!(env_val(&p, "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"), Some("1"));
        assert_eq!(env_val(&p, "API_TIMEOUT_MS"), Some("600000"));
    }

    #[test]
    fn test_zai_preset() {
        let p = get_preset("zai");
        assert_eq!(p.base_url.as_deref(), Some("https://api.z.ai/api/anthropic"));
        assert_eq!(p.auth_token.as_deref(), Some("${ZAI_API_KEY}"));
        assert_eq!(models(&p).haiku.as_deref(), Some("GLM-4.5-Air"));
        assert_eq!(models(&p).sonnet.as_deref(), Some("GLM-4.7"));
        assert_eq!(models(&p).opus.as_deref(), Some("GLM-4.7"));
        assert_eq!(env_val(&p, "API_TIMEOUT_MS"), Some("3000000"));
    }

    #[test]
    fn test_alibaba_preset() {
        let p = get_preset("alibaba");
        assert_eq!(
            p.base_url.as_deref(),
            Some("https://coding-intl.dashscope.aliyuncs.com/apps/anthropic")
        );
        assert_eq!(p.auth_token.as_deref(), Some("${ALIBABA_API_KEY}"));
        assert_eq!(models(&p).haiku.as_deref(), Some("qwen3.5-plus"));
        assert_eq!(models(&p).sonnet.as_deref(), Some("kimi-k2.5"));
        assert_eq!(models(&p).opus.as_deref(), Some("glm-5"));
        assert_eq!(env_val(&p, "ANTHROPIC_MODEL"), Some("kimi-k2.5"));
        assert_eq!(env_val(&p, "ANTHROPIC_SMALL_FAST_MODEL"), Some("qwen3.5-plus"));
    }

    // ── description coverage ─────────────────────────────────────────────────

    #[test]
    fn test_all_preset_names_have_description() {
        for name in PRESET_NAMES {
            let desc = get_preset_description(name);
            assert_ne!(desc, "Unknown provider", "preset '{name}' has no description");
        }
    }
}

use crate::config::{Models, Profile};

pub const PRESET_NAMES: &[&str] = &["ollama", "openrouter", "gemini", "openai", "custom"];

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
                sonnet: Some("gpt-4o".into()),
                opus: Some("o3".into()),
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
        "custom" => "Custom provider — fill in manually",
        _ => "Unknown provider",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_names_all_exist() {
        for name in PRESET_NAMES {
            let preset = get_preset(name);
            // All presets except custom should have a base_url
            if *name != "custom" {
                assert!(
                    preset.base_url.is_some(),
                    "preset '{name}' missing base_url"
                );
            }
        }
    }

    #[test]
    fn test_ollama_preset() {
        let p = get_preset("ollama");
        assert_eq!(p.base_url.as_deref(), Some("http://localhost:11434/v1"));
        assert_eq!(p.auth_token.as_deref(), Some("ollama"));
        let models = p.models.unwrap();
        assert!(models.haiku.is_some());
        assert!(models.sonnet.is_some());
        assert!(models.opus.is_some());
    }

    #[test]
    fn test_get_preset_description() {
        assert!(get_preset_description("ollama").contains("Ollama"));
        assert!(get_preset_description("openrouter").contains("OpenRouter"));
    }

    #[test]
    fn test_custom_preset_is_empty() {
        let p = get_preset("custom");
        assert!(p.base_url.is_none());
        assert!(p.auth_token.is_none());
    }
}

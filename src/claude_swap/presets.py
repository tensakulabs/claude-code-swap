"""Provider preset definitions for interactive profile creation."""

from __future__ import annotations

PRESETS: dict[str, dict] = {
    "ollama": {
        "_description": "Local Ollama instance (no API key needed)",
        "_test_endpoint": "/api/version",
        "base_url": "http://localhost:11434/v1",
        "auth_token": "ollama",
        "models": {
            "haiku": "llama3.2:3b",
            "sonnet": "llama3.1:8b",
            "opus": "llama3.1:70b",
        },
    },
    "openrouter": {
        "_description": "OpenRouter (set OPENROUTER_API_KEY)",
        "base_url": "https://openrouter.ai/api/v1",
        "auth_token": "${OPENROUTER_API_KEY}",
        "api_key": "",
        "models": {
            "haiku": "qwen/qwen3-4b:free",
            "sonnet": "qwen/qwen3-coder:free",
            "opus": "deepseek/deepseek-r1-0528:free",
        },
    },
    "gemini": {
        "_description": "Google Gemini via OpenAI-compat endpoint (set GEMINI_API_KEY)",
        "base_url": "https://generativelanguage.googleapis.com/v1beta/openai",
        "auth_token": "${GEMINI_API_KEY}",
        "api_key": "",
        "models": {
            "haiku": "gemini-2.0-flash",
            "sonnet": "gemini-2.5-flash",
            "opus": "gemini-2.5-pro",
        },
    },
    "openai": {
        "_description": "OpenAI direct (set OPENAI_API_KEY)",
        "base_url": "https://api.openai.com/v1",
        "auth_token": "${OPENAI_API_KEY}",
        "api_key": "",
        "models": {
            "haiku": "gpt-4o-mini",
            "sonnet": "gpt-4o",
            "opus": "o3",
        },
    },
    "custom": {
        "_description": "Custom provider — fill in manually",
    },
}


def get_preset(name: str) -> dict:
    """Return a preset dict stripped of private _ metadata keys."""
    preset = PRESETS.get(name, {})
    return {k: v for k, v in preset.items() if not k.startswith("_")}


def get_preset_description(name: str) -> str:
    """Return the human-readable description for a preset."""
    return PRESETS.get(name, {}).get("_description", f"{name} provider")


PRESET_NAMES = list(PRESETS.keys())

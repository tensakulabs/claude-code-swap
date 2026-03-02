"""Integration tests: full config.yaml → environment variable pipeline."""

from __future__ import annotations

import os

import pytest


# ---------------------------------------------------------------------------
# Full pipeline: profile → resolved env → ANTHROPIC_* vars
# ---------------------------------------------------------------------------


def test_default_profile_produces_no_overrides():
    from claude_swap.config import build_env_overrides, resolve_profile_env

    result = build_env_overrides(resolve_profile_env({}))
    assert result == {}


def test_openrouter_profile_pipeline(monkeypatch):
    monkeypatch.setenv("OPENROUTER_API_KEY", "sk-or-testkey12345678901234567890")
    from claude_swap.config import build_env_overrides, resolve_profile_env

    profile = {
        "base_url": "https://openrouter.ai/api/v1",
        "auth_token": "${OPENROUTER_API_KEY}",
        "api_key": "",
        "models": {
            "haiku": "qwen/qwen3-4b:free",
            "sonnet": "qwen/qwen3-coder:free",
            "opus": "deepseek/deepseek-r1-0528:free",
        },
    }

    resolved = resolve_profile_env(profile)
    assert resolved["auth_token"] == "sk-or-testkey12345678901234567890"

    overrides = build_env_overrides(resolved)
    assert overrides["ANTHROPIC_BASE_URL"] == "https://openrouter.ai/api/v1"
    assert overrides["ANTHROPIC_AUTH_TOKEN"] == "sk-or-testkey12345678901234567890"
    assert overrides["ANTHROPIC_API_KEY"] == ""
    assert overrides["ANTHROPIC_DEFAULT_HAIKU_MODEL"] == "qwen/qwen3-4b:free"
    assert overrides["ANTHROPIC_DEFAULT_SONNET_MODEL"] == "qwen/qwen3-coder:free"
    assert overrides["ANTHROPIC_DEFAULT_OPUS_MODEL"] == "deepseek/deepseek-r1-0528:free"


def test_ollama_profile_pipeline():
    from claude_swap.config import build_env_overrides, resolve_profile_env

    profile = {
        "base_url": "http://localhost:11434/v1",
        "auth_token": "ollama",
        "models": {
            "haiku": "llama3.2:3b",
            "sonnet": "llama3.1:8b",
            "opus": "llama3.1:70b",
        },
    }

    resolved = resolve_profile_env(profile)
    overrides = build_env_overrides(resolved)

    assert overrides["ANTHROPIC_BASE_URL"] == "http://localhost:11434/v1"
    assert overrides["ANTHROPIC_AUTH_TOKEN"] == "ollama"
    assert overrides["ANTHROPIC_DEFAULT_SONNET_MODEL"] == "llama3.1:8b"


def test_partial_models_pipeline():
    """Profile with only sonnet model should only set sonnet env var."""
    from claude_swap.config import build_env_overrides, resolve_profile_env

    profile = {
        "base_url": "http://x.com",
        "models": {"sonnet": "model-x"},
    }

    overrides = build_env_overrides(resolve_profile_env(profile))
    assert "ANTHROPIC_DEFAULT_SONNET_MODEL" in overrides
    assert "ANTHROPIC_DEFAULT_HAIKU_MODEL" not in overrides
    assert "ANTHROPIC_DEFAULT_OPUS_MODEL" not in overrides


def test_missing_env_ref_raises_on_resolution(monkeypatch):
    monkeypatch.delenv("MISSING_KEY", raising=False)
    from claude_swap.config import resolve_profile_env

    profile = {"auth_token": "${MISSING_KEY}"}
    with pytest.raises(ValueError, match="MISSING_KEY"):
        resolve_profile_env(profile)


def test_env_ref_in_model_field(monkeypatch):
    monkeypatch.setenv("MY_MODEL", "custom/model-v1")
    from claude_swap.config import build_env_overrides, resolve_profile_env

    profile = {
        "base_url": "http://x.com",
        "models": {"sonnet": "${MY_MODEL}"},
    }

    resolved = resolve_profile_env(profile)
    overrides = build_env_overrides(resolved)
    assert overrides["ANTHROPIC_DEFAULT_SONNET_MODEL"] == "custom/model-v1"


def test_extra_env_passthrough_pipeline():
    """Extra env vars from profile.env are passed through to overrides."""
    from claude_swap.config import build_env_overrides, resolve_profile_env

    profile = {
        "base_url": "http://localhost:11434/v1",
        "env": {
            "OLLAMA_CONTEXT_LENGTH": "64000",
            "OLLAMA_MAX_LOADED_MODELS": "3",
        },
    }

    overrides = build_env_overrides(resolve_profile_env(profile))
    assert overrides["OLLAMA_CONTEXT_LENGTH"] == "64000"
    assert overrides["OLLAMA_MAX_LOADED_MODELS"] == "3"


def test_build_env_in_launcher_integrates_correctly(monkeypatch):
    """Full integration: launcher.build_env produces correct combined env."""
    monkeypatch.setenv("SOME_EXISTING", "existing-value")
    monkeypatch.delenv("ANTHROPIC_BASE_URL", raising=False)

    from claude_swap.launcher import build_env

    profile = {"base_url": "http://x.com", "auth_token": "tok"}
    env = build_env(profile)

    assert env["ANTHROPIC_BASE_URL"] == "http://x.com"
    assert env["SOME_EXISTING"] == "existing-value"
    assert "ANTHROPIC_BASE_URL" not in os.environ  # original not modified

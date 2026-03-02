"""Tests for config.py: YAML loading, ${VAR} resolution, env mapping, security warnings."""

from __future__ import annotations

import os
from pathlib import Path
from unittest.mock import patch

import pytest
import yaml


# ---------------------------------------------------------------------------
# load_config
# ---------------------------------------------------------------------------


def test_load_config_missing_file(tmp_path, monkeypatch):
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", tmp_path / "nonexistent.yaml")
    from claude_swap.config import load_config

    assert load_config() == {}


def test_load_config_empty_file(tmp_path, monkeypatch):
    cfg = tmp_path / "config.yaml"
    cfg.write_text("")
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", cfg)
    from claude_swap.config import load_config

    assert load_config() == {}


def test_load_config_valid_yaml(tmp_path, monkeypatch):
    cfg = tmp_path / "config.yaml"
    cfg.write_text("profiles:\n  default: {}\n  ollama:\n    base_url: http://localhost\n")
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", cfg)
    from claude_swap.config import load_config

    result = load_config()
    assert "profiles" in result
    assert "ollama" in result["profiles"]


def test_load_config_invalid_yaml_raises(tmp_path, monkeypatch):
    cfg = tmp_path / "config.yaml"
    cfg.write_text(": bad: yaml: [unclosed")
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", cfg)
    from claude_swap.config import load_config

    with pytest.raises(yaml.YAMLError):
        load_config()


# ---------------------------------------------------------------------------
# resolve_env_refs
# ---------------------------------------------------------------------------


def test_resolve_env_refs_expands(monkeypatch):
    monkeypatch.setenv("MY_KEY", "abc123")
    from claude_swap.config import resolve_env_refs

    assert resolve_env_refs("${MY_KEY}") == "abc123"


def test_resolve_env_refs_noop_on_plain_string(monkeypatch):
    from claude_swap.config import resolve_env_refs

    assert resolve_env_refs("plain string") == "plain string"


def test_resolve_env_refs_multiple_refs(monkeypatch):
    monkeypatch.setenv("HOST", "localhost")
    monkeypatch.setenv("PORT", "8080")
    from claude_swap.config import resolve_env_refs

    assert resolve_env_refs("${HOST}:${PORT}") == "localhost:8080"


def test_resolve_env_refs_missing_raises(monkeypatch):
    monkeypatch.delenv("MISSING_VAR", raising=False)
    from claude_swap.config import resolve_env_refs

    with pytest.raises(ValueError, match="MISSING_VAR"):
        resolve_env_refs("${MISSING_VAR}")


# ---------------------------------------------------------------------------
# resolve_profile_env
# ---------------------------------------------------------------------------


def test_resolve_profile_env_nested(monkeypatch):
    monkeypatch.setenv("OR_KEY", "key-abc")
    from claude_swap.config import resolve_profile_env

    profile = {
        "base_url": "https://openrouter.ai",
        "auth_token": "${OR_KEY}",
        "models": {"sonnet": "model-x"},
    }
    resolved = resolve_profile_env(profile)
    assert resolved["auth_token"] == "key-abc"
    assert resolved["base_url"] == "https://openrouter.ai"
    assert resolved["models"]["sonnet"] == "model-x"


def test_resolve_profile_env_empty():
    from claude_swap.config import resolve_profile_env

    assert resolve_profile_env({}) == {}


# ---------------------------------------------------------------------------
# build_env_overrides
# ---------------------------------------------------------------------------


def test_build_env_overrides_full_profile():
    from claude_swap.config import build_env_overrides

    profile = {
        "base_url": "https://example.com",
        "auth_token": "token123",
        "api_key": "",
        "models": {
            "haiku": "fast-model",
            "sonnet": "big-model",
            "opus": "biggest-model",
        },
    }
    result = build_env_overrides(profile)
    assert result["ANTHROPIC_BASE_URL"] == "https://example.com"
    assert result["ANTHROPIC_AUTH_TOKEN"] == "token123"
    assert result["ANTHROPIC_API_KEY"] == ""
    assert result["ANTHROPIC_DEFAULT_HAIKU_MODEL"] == "fast-model"
    assert result["ANTHROPIC_DEFAULT_SONNET_MODEL"] == "big-model"
    assert result["ANTHROPIC_DEFAULT_OPUS_MODEL"] == "biggest-model"


def test_build_env_overrides_partial_profile():
    from claude_swap.config import build_env_overrides

    profile = {"base_url": "https://x.com"}
    result = build_env_overrides(profile)
    assert result["ANTHROPIC_BASE_URL"] == "https://x.com"
    assert "ANTHROPIC_AUTH_TOKEN" not in result
    assert "ANTHROPIC_DEFAULT_SONNET_MODEL" not in result


def test_build_env_overrides_empty_profile():
    from claude_swap.config import build_env_overrides

    assert build_env_overrides({}) == {}


def test_build_env_overrides_none_values_excluded():
    from claude_swap.config import build_env_overrides

    profile = {"base_url": None, "auth_token": "tok"}
    result = build_env_overrides(profile)
    assert "ANTHROPIC_BASE_URL" not in result
    assert result["ANTHROPIC_AUTH_TOKEN"] == "tok"


def test_build_env_overrides_extra_env_passthrough():
    from claude_swap.config import build_env_overrides

    profile = {
        "base_url": "http://x",
        "env": {"OLLAMA_CONTEXT_LENGTH": "64000", "MY_CUSTOM": "value"},
    }
    result = build_env_overrides(profile)
    assert result["OLLAMA_CONTEXT_LENGTH"] == "64000"
    assert result["MY_CUSTOM"] == "value"


# ---------------------------------------------------------------------------
# warn_hardcoded_keys
# ---------------------------------------------------------------------------


def test_warn_hardcoded_keys_detects_sk_key(capsys):
    from claude_swap.config import warn_hardcoded_keys

    config = {
        "profiles": {
            "bad": {"auth_token": "sk-abcdefghijklmnopqrstuvwxyz1234567890"}
        }
    }
    with patch("sys.stderr.isatty", return_value=False):
        warn_hardcoded_keys(config)
    captured = capsys.readouterr()
    assert "warning" in captured.err.lower()


def test_warn_hardcoded_keys_no_warn_on_env_ref(capsys):
    from claude_swap.config import warn_hardcoded_keys

    config = {
        "profiles": {
            "good": {"auth_token": "${OPENROUTER_API_KEY}"}
        }
    }
    warn_hardcoded_keys(config)
    captured = capsys.readouterr()
    assert "warning" not in captured.err.lower()


def test_warn_hardcoded_keys_no_warn_on_empty():
    from claude_swap.config import warn_hardcoded_keys

    warn_hardcoded_keys({})  # should not raise


# ---------------------------------------------------------------------------
# save_config (atomic write)
# ---------------------------------------------------------------------------


def test_save_config_round_trip(tmp_path, monkeypatch):
    cfg = tmp_path / "config.yaml"
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", cfg)
    monkeypatch.setattr("claude_swap.config.CONFIG_DIR", tmp_path)
    from claude_swap.config import save_config, load_config

    data = {"profiles": {"default": {}, "test": {"base_url": "http://x"}}}
    save_config(data, cfg)

    assert cfg.exists()
    loaded = load_config()
    assert loaded["profiles"]["test"]["base_url"] == "http://x"


def test_save_config_sets_permissions(tmp_path, monkeypatch):
    import stat

    cfg = tmp_path / "config.yaml"
    monkeypatch.setattr("claude_swap.config.CONFIG_DIR", tmp_path)
    from claude_swap.config import save_config

    save_config({"profiles": {}}, cfg)
    mode = stat.S_IMODE(cfg.stat().st_mode)
    assert mode == 0o600


def test_save_config_no_tmp_file_remains(tmp_path, monkeypatch):
    cfg = tmp_path / "config.yaml"
    monkeypatch.setattr("claude_swap.config.CONFIG_DIR", tmp_path)
    from claude_swap.config import save_config

    save_config({}, cfg)
    tmp = cfg.with_suffix(".yaml.tmp")
    assert not tmp.exists()

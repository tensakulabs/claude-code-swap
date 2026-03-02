"""Tests for state.py: active profile read/write."""

from __future__ import annotations

import stat
from pathlib import Path

import pytest
import yaml


def _patch_state(monkeypatch, tmp_path: Path):
    """Redirect STATE_FILE and CONFIG_DIR to tmp_path for isolation."""
    state_file = tmp_path / "state.yaml"
    monkeypatch.setattr("claude_swap.state.STATE_FILE", state_file)
    monkeypatch.setattr("claude_swap.state.CONFIG_DIR", tmp_path)
    monkeypatch.setattr("claude_swap.config.CONFIG_DIR", tmp_path)
    return state_file


# ---------------------------------------------------------------------------
# get_active_profile
# ---------------------------------------------------------------------------


def test_get_active_profile_default_when_missing(monkeypatch, tmp_path):
    _patch_state(monkeypatch, tmp_path)
    from claude_swap.state import get_active_profile

    assert get_active_profile() == "default"


def test_get_active_profile_reads_stored_value(monkeypatch, tmp_path):
    state_file = _patch_state(monkeypatch, tmp_path)
    state_file.write_text("active: ollama\n")
    from claude_swap.state import get_active_profile

    assert get_active_profile() == "ollama"


def test_get_active_profile_fallback_on_corrupt(monkeypatch, tmp_path):
    state_file = _patch_state(monkeypatch, tmp_path)
    state_file.write_text(": bad yaml [")
    from claude_swap.state import get_active_profile

    # Should not raise — returns default
    assert get_active_profile() == "default"


def test_get_active_profile_fallback_on_empty(monkeypatch, tmp_path):
    state_file = _patch_state(monkeypatch, tmp_path)
    state_file.write_text("")
    from claude_swap.state import get_active_profile

    assert get_active_profile() == "default"


# ---------------------------------------------------------------------------
# set_active_profile
# ---------------------------------------------------------------------------


def test_set_active_profile_writes(monkeypatch, tmp_path):
    state_file = _patch_state(monkeypatch, tmp_path)
    from claude_swap.state import set_active_profile, get_active_profile

    set_active_profile("openrouter")
    assert get_active_profile() == "openrouter"


def test_set_active_profile_sets_permissions(monkeypatch, tmp_path):
    state_file = _patch_state(monkeypatch, tmp_path)
    from claude_swap.state import set_active_profile

    set_active_profile("ollama")
    mode = stat.S_IMODE(state_file.stat().st_mode)
    assert mode == 0o600


def test_set_active_profile_no_tmp_remains(monkeypatch, tmp_path):
    state_file = _patch_state(monkeypatch, tmp_path)
    from claude_swap.state import set_active_profile

    set_active_profile("gemini")
    tmp = state_file.with_suffix(".yaml.tmp")
    assert not tmp.exists()


def test_set_active_profile_overwrites(monkeypatch, tmp_path):
    state_file = _patch_state(monkeypatch, tmp_path)
    from claude_swap.state import set_active_profile, get_active_profile

    set_active_profile("ollama")
    set_active_profile("openrouter")
    assert get_active_profile() == "openrouter"

"""Tests for doctor.py: individual health check functions."""

from __future__ import annotations

import stat
from pathlib import Path
from unittest.mock import patch

import pytest
import yaml


def _patch_doctor(monkeypatch, tmp_path: Path):
    monkeypatch.setattr("claude_swap.config.CONFIG_DIR", tmp_path)
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", tmp_path / "config.yaml")
    monkeypatch.setattr("claude_swap.state.STATE_FILE", tmp_path / "state.yaml")
    monkeypatch.setattr("claude_swap.state.CONFIG_DIR", tmp_path)
    monkeypatch.setattr("claude_swap.doctor.CONFIG_DIR", tmp_path)
    monkeypatch.setattr("claude_swap.doctor.CONFIG_FILE", tmp_path / "config.yaml")
    monkeypatch.setattr("claude_swap.doctor.STATE_FILE", tmp_path / "state.yaml")


# ---------------------------------------------------------------------------
# check_claude_binary
# ---------------------------------------------------------------------------


def test_check_claude_binary_found_via_env(monkeypatch, tmp_path):
    monkeypatch.setenv("CLAUDE_BINARY", "/path/to/claude")
    from claude_swap.doctor import check_claude_binary

    result = check_claude_binary()
    assert result.status == "ok"
    assert "/path/to/claude" in result.message


def test_check_claude_binary_found_via_which(monkeypatch):
    monkeypatch.delenv("CLAUDE_BINARY", raising=False)
    with patch("shutil.which", return_value="/usr/local/bin/claude"):
        from claude_swap.doctor import check_claude_binary

        result = check_claude_binary()
    assert result.status == "ok"


def test_check_claude_binary_not_found(monkeypatch):
    monkeypatch.delenv("CLAUDE_BINARY", raising=False)
    with patch("shutil.which", return_value=None):
        from claude_swap.doctor import check_claude_binary

        result = check_claude_binary()
    assert result.status == "fail"
    assert "install" in result.message.lower() or "not found" in result.message.lower()


# ---------------------------------------------------------------------------
# check_config_exists
# ---------------------------------------------------------------------------


def test_check_config_exists_missing(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    from claude_swap.doctor import check_config_exists

    result = check_config_exists()
    assert result.status == "warn"


def test_check_config_exists_present(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    (tmp_path / "config.yaml").write_text("profiles:\n  default: {}\n")
    from claude_swap.doctor import check_config_exists

    result = check_config_exists()
    assert result.status == "ok"


# ---------------------------------------------------------------------------
# check_config_parseable
# ---------------------------------------------------------------------------


def test_check_config_parseable_missing(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    from claude_swap.doctor import check_config_parseable

    result = check_config_parseable()
    assert result.status == "warn"


def test_check_config_parseable_valid(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    (tmp_path / "config.yaml").write_text(
        "profiles:\n  default: {}\n  ollama:\n    base_url: http://localhost\n"
    )
    from claude_swap.doctor import check_config_parseable

    result = check_config_parseable()
    assert result.status == "ok"
    assert "2" in result.message  # 2 profiles


def test_check_config_parseable_invalid(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    (tmp_path / "config.yaml").write_text(": bad yaml [unclosed")
    from claude_swap.doctor import check_config_parseable

    result = check_config_parseable()
    assert result.status == "fail"


# ---------------------------------------------------------------------------
# check_config_dir_permissions
# ---------------------------------------------------------------------------


def test_check_config_dir_permissions_missing(monkeypatch, tmp_path):
    nonexistent = tmp_path / "nonexistent-dir"
    _patch_doctor(monkeypatch, tmp_path)
    monkeypatch.setattr("claude_swap.doctor.CONFIG_DIR", nonexistent)
    from claude_swap.doctor import check_config_dir_permissions

    result = check_config_dir_permissions()
    assert result.status == "warn"


def test_check_config_dir_permissions_correct(monkeypatch, tmp_path):
    d = tmp_path / "claude-code-swap"
    d.mkdir(mode=0o700)
    _patch_doctor(monkeypatch, tmp_path)
    monkeypatch.setattr("claude_swap.doctor.CONFIG_DIR", d)
    from claude_swap.doctor import check_config_dir_permissions

    result = check_config_dir_permissions()
    assert result.status == "ok"


# ---------------------------------------------------------------------------
# check_active_profile_valid
# ---------------------------------------------------------------------------


def test_check_active_profile_valid_default_no_config(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    # No state file → falls back to "default"; no config → ok
    from claude_swap.doctor import check_active_profile_valid

    result = check_active_profile_valid()
    assert result.status == "ok"


def test_check_active_profile_valid_exists(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    (tmp_path / "config.yaml").write_text(
        "profiles:\n  default: {}\n  ollama:\n    base_url: http://x\n"
    )
    (tmp_path / "state.yaml").write_text("active: ollama\n")
    from claude_swap.doctor import check_active_profile_valid

    result = check_active_profile_valid()
    assert result.status == "ok"
    assert "ollama" in result.message


def test_check_active_profile_valid_stale_profile(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    (tmp_path / "config.yaml").write_text(
        "profiles:\n  default: {}\n  ollama:\n    base_url: http://x\n"
    )
    # State points to a profile that doesn't exist
    (tmp_path / "state.yaml").write_text("active: deleted-profile\n")
    from claude_swap.doctor import check_active_profile_valid

    result = check_active_profile_valid()
    assert result.status == "fail"


# ---------------------------------------------------------------------------
# check_env_refs_resolvable
# ---------------------------------------------------------------------------


def test_check_env_refs_all_resolved(monkeypatch, tmp_path):
    monkeypatch.setenv("OPENROUTER_API_KEY", "test-key")
    _patch_doctor(monkeypatch, tmp_path)
    (tmp_path / "config.yaml").write_text(
        "profiles:\n  openrouter:\n    auth_token: ${OPENROUTER_API_KEY}\n"
    )
    (tmp_path / "state.yaml").write_text("active: openrouter\n")
    from claude_swap.doctor import check_env_refs_resolvable

    result = check_env_refs_resolvable()
    assert result.status == "ok"


def test_check_env_refs_missing_var(monkeypatch, tmp_path):
    monkeypatch.delenv("MISSING_KEY", raising=False)
    _patch_doctor(monkeypatch, tmp_path)
    (tmp_path / "config.yaml").write_text(
        "profiles:\n  bad:\n    auth_token: ${MISSING_KEY}\n"
    )
    (tmp_path / "state.yaml").write_text("active: bad\n")
    from claude_swap.doctor import check_env_refs_resolvable

    result = check_env_refs_resolvable()
    assert result.status == "fail"
    assert "MISSING_KEY" in result.message


def test_check_env_refs_default_profile_skipped(monkeypatch, tmp_path):
    _patch_doctor(monkeypatch, tmp_path)
    (tmp_path / "config.yaml").write_text("profiles:\n  default: {}\n")
    (tmp_path / "state.yaml").write_text("active: default\n")
    from claude_swap.doctor import check_env_refs_resolvable

    result = check_env_refs_resolvable()
    assert result.status == "ok"


# ---------------------------------------------------------------------------
# run_doctor (full integration)
# ---------------------------------------------------------------------------


def test_run_doctor_output_format(monkeypatch, tmp_path, capsys):
    _patch_doctor(monkeypatch, tmp_path)
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    (tmp_path / "config.yaml").write_text("profiles:\n  default: {}\n")
    (tmp_path / "state.yaml").write_text("active: default\n")

    from claude_swap.doctor import run_doctor

    run_doctor()
    captured = capsys.readouterr()

    # Must show version
    from claude_swap import __version__

    assert __version__ in captured.out

    # Must show check results
    lines = captured.out.splitlines()
    check_lines = [l for l in lines if "[OK]" in l or "[WARN]" in l or "[FAIL]" in l]
    assert len(check_lines) >= 4, f"Expected at least 4 check result lines, got: {check_lines}"

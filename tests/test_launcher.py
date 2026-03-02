"""Tests for launcher.py: find_claude_binary, build_env, launch."""

from __future__ import annotations

import os
from unittest.mock import patch

import pytest


# ---------------------------------------------------------------------------
# find_claude_binary
# ---------------------------------------------------------------------------


def test_find_claude_binary_via_env_var(monkeypatch):
    monkeypatch.setenv("CLAUDE_BINARY", "/custom/path/to/claude")
    from claude_swap.launcher import find_claude_binary

    assert find_claude_binary() == "/custom/path/to/claude"


def test_find_claude_binary_via_which(monkeypatch):
    monkeypatch.delenv("CLAUDE_BINARY", raising=False)
    with patch("shutil.which", return_value="/usr/local/bin/claude"):
        from claude_swap.launcher import find_claude_binary
        import importlib
        import claude_swap.launcher as launcher_mod
        importlib.reload(launcher_mod)

        assert launcher_mod.find_claude_binary() == "/usr/local/bin/claude"


def test_find_claude_binary_not_found_raises(monkeypatch):
    monkeypatch.delenv("CLAUDE_BINARY", raising=False)
    with patch("shutil.which", return_value=None):
        import importlib
        import claude_swap.launcher as launcher_mod
        importlib.reload(launcher_mod)

        with pytest.raises(FileNotFoundError, match="Claude Code"):
            launcher_mod.find_claude_binary()


# ---------------------------------------------------------------------------
# build_env
# ---------------------------------------------------------------------------


def test_build_env_includes_existing_env(monkeypatch):
    monkeypatch.setenv("EXISTING_VAR", "keep-me")
    monkeypatch.delenv("ANTHROPIC_BASE_URL", raising=False)

    from claude_swap.launcher import build_env

    env = build_env({})
    assert "EXISTING_VAR" in env
    assert env["EXISTING_VAR"] == "keep-me"


def test_build_env_overlays_profile_overrides(monkeypatch):
    monkeypatch.delenv("ANTHROPIC_BASE_URL", raising=False)
    from claude_swap.launcher import build_env

    profile = {"base_url": "http://localhost:11434/v1", "auth_token": "ollama"}
    env = build_env(profile)
    assert env["ANTHROPIC_BASE_URL"] == "http://localhost:11434/v1"
    assert env["ANTHROPIC_AUTH_TOKEN"] == "ollama"


def test_build_env_does_not_mutate_os_environ(monkeypatch):
    monkeypatch.delenv("ANTHROPIC_BASE_URL", raising=False)
    from claude_swap.launcher import build_env

    build_env({"base_url": "http://x.com"})
    assert "ANTHROPIC_BASE_URL" not in os.environ


def test_build_env_empty_profile_preserves_existing(monkeypatch):
    monkeypatch.setenv("HOME", "/home/user")
    from claude_swap.launcher import build_env

    env = build_env({})
    assert env.get("HOME") == "/home/user"


# ---------------------------------------------------------------------------
# launch
# ---------------------------------------------------------------------------


def test_launch_calls_execvpe_correctly(monkeypatch):
    captured = {}

    def mock_execvpe(path, args, env):
        captured["path"] = path
        captured["args"] = args
        captured["env"] = env

    monkeypatch.setattr(os, "execvpe", mock_execvpe)
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    monkeypatch.delenv("ANTHROPIC_BASE_URL", raising=False)

    import importlib
    import claude_swap.launcher as launcher_mod
    importlib.reload(launcher_mod)

    profile = {"base_url": "http://localhost:11434/v1", "auth_token": "test"}
    launcher_mod.launch(profile, ["--debug"])

    assert captured["path"] == "/fake/claude"
    assert captured["args"] == ["/fake/claude", "--debug"]
    assert captured["env"]["ANTHROPIC_BASE_URL"] == "http://localhost:11434/v1"


def test_launch_with_no_extra_args(monkeypatch):
    captured = {}

    def mock_execvpe(path, args, env):
        captured["path"] = path
        captured["args"] = args

    monkeypatch.setattr(os, "execvpe", mock_execvpe)
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")

    import importlib
    import claude_swap.launcher as launcher_mod
    importlib.reload(launcher_mod)

    launcher_mod.launch({}, [])
    assert captured["args"] == ["/fake/claude"]


def test_launch_resolves_env_refs(monkeypatch):
    captured = {}

    def mock_execvpe(path, args, env):
        captured["env"] = env

    monkeypatch.setattr(os, "execvpe", mock_execvpe)
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    monkeypatch.setenv("MY_TOKEN", "resolved-token")
    monkeypatch.delenv("ANTHROPIC_AUTH_TOKEN", raising=False)

    import importlib
    import claude_swap.launcher as launcher_mod
    importlib.reload(launcher_mod)

    launcher_mod.launch({"auth_token": "${MY_TOKEN}"}, [])
    assert captured["env"]["ANTHROPIC_AUTH_TOKEN"] == "resolved-token"


def test_launch_raises_if_claude_not_found(monkeypatch):
    monkeypatch.delenv("CLAUDE_BINARY", raising=False)
    with patch("shutil.which", return_value=None):
        import importlib
        import claude_swap.launcher as launcher_mod
        importlib.reload(launcher_mod)

        with pytest.raises(FileNotFoundError):
            launcher_mod.launch({}, [])

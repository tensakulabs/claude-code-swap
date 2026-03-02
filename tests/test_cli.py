"""Integration tests for cli.py — all command surfaces."""

from __future__ import annotations

import os
import sys
from pathlib import Path
from unittest.mock import patch

import pytest
import yaml


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _run_cli(args: list[str], monkeypatch, tmp_path: Path):
    """
    Invoke cli.main() with patched sys.argv and isolated config dir.
    Returns (exit_code, stdout, stderr) via capsys.
    Raises SystemExit and re-raises so callers can catch it.
    """
    monkeypatch.setattr(sys, "argv", ["ccs"] + args)
    monkeypatch.setattr("claude_swap.config.CONFIG_DIR", tmp_path)
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", tmp_path / "config.yaml")
    monkeypatch.setattr("claude_swap.state.STATE_FILE", tmp_path / "state.yaml")
    monkeypatch.setattr("claude_swap.state.CONFIG_DIR", tmp_path)


def _write_config(tmp_path: Path, config: dict) -> None:
    import yaml

    cfg = tmp_path / "config.yaml"
    cfg.write_text(yaml.dump(config, default_flow_style=False))


def _write_state(tmp_path: Path, active: str) -> None:
    import yaml

    state = tmp_path / "state.yaml"
    state.write_text(yaml.dump({"active": active}))


SAMPLE_CONFIG = {
    "profiles": {
        "default": {},
        "ollama": {
            "base_url": "http://localhost:11434/v1",
            "auth_token": "ollama",
            "models": {"haiku": "llama3.2:3b", "sonnet": "llama3.1:8b"},
        },
        "openrouter": {
            "base_url": "https://openrouter.ai/api/v1",
            "auth_token": "${OPENROUTER_API_KEY}",
            "models": {"sonnet": "qwen/qwen3-coder:free"},
        },
    }
}


# ---------------------------------------------------------------------------
# ccs --version
# ---------------------------------------------------------------------------


def test_version(monkeypatch, tmp_path, capsys):
    from claude_swap import __version__
    from claude_swap.cli import create_parser

    parser = create_parser()
    with pytest.raises(SystemExit) as exc_info:
        parser.parse_args(["--version"])
    assert exc_info.value.code == 0
    captured = capsys.readouterr()
    assert __version__ in captured.out


# ---------------------------------------------------------------------------
# ccs status
# ---------------------------------------------------------------------------


def test_status_default_no_config(monkeypatch, tmp_path, capsys):
    _run_cli(["status"], monkeypatch, tmp_path)
    from claude_swap.cli import cmd_status
    import argparse

    cmd_status(argparse.Namespace())
    captured = capsys.readouterr()
    assert "Active profile" in captured.out
    assert "default" in captured.out


def test_status_with_active_profile(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "ollama")
    _run_cli(["status"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_status
    import argparse

    cmd_status(argparse.Namespace())
    captured = capsys.readouterr()
    assert "ollama" in captured.out
    assert "localhost:11434" in captured.out


# ---------------------------------------------------------------------------
# ccs use
# ---------------------------------------------------------------------------


def test_use_valid_profile(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _run_cli(["use", "ollama"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_use
    import argparse

    cmd_use(argparse.Namespace(profile="ollama"))
    captured = capsys.readouterr()
    assert "Switched to: ollama" in captured.out

    # Verify state was actually written
    from claude_swap.state import get_active_profile

    assert get_active_profile() == "ollama"


def test_use_nonexistent_profile_exits_1(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _run_cli(["use", "nonexistent"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_use
    import argparse

    with pytest.raises(SystemExit) as exc_info:
        cmd_use(argparse.Namespace(profile="nonexistent"))
    assert exc_info.value.code == 1


def test_use_default_always_works(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _run_cli(["use", "default"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_use
    import argparse

    cmd_use(argparse.Namespace(profile="default"))
    captured = capsys.readouterr()
    assert "default" in captured.out


# ---------------------------------------------------------------------------
# ccs profile list
# ---------------------------------------------------------------------------


def test_profile_list(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "ollama")
    _run_cli(["profile", "list"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_profile_list

    cmd_profile_list(SAMPLE_CONFIG)
    captured = capsys.readouterr()
    assert "default" in captured.out
    assert "ollama" in captured.out
    assert "openrouter" in captured.out
    # Active profile marked with *
    assert "*" in captured.out


def test_profile_list_empty_config(monkeypatch, tmp_path, capsys):
    _run_cli(["profile", "list"], monkeypatch, tmp_path)
    from claude_swap.cli import cmd_profile_list

    cmd_profile_list({})
    captured = capsys.readouterr()
    assert "default" in captured.out


# ---------------------------------------------------------------------------
# ccs profile show
# ---------------------------------------------------------------------------


def test_profile_show_default(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _run_cli(["profile", "show", "default"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_profile_show

    cmd_profile_show(SAMPLE_CONFIG, "default")
    captured = capsys.readouterr()
    assert "Profile: default" in captured.out


def test_profile_show_named(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _run_cli(["profile", "show", "ollama"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_profile_show

    cmd_profile_show(SAMPLE_CONFIG, "ollama")
    captured = capsys.readouterr()
    assert "Profile: ollama" in captured.out
    assert "localhost" in captured.out


def test_profile_show_nonexistent_exits_1(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _run_cli(["profile", "show", "nonexistent"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_profile_show

    with pytest.raises(SystemExit) as exc_info:
        cmd_profile_show(SAMPLE_CONFIG, "nonexistent")
    assert exc_info.value.code == 1


# ---------------------------------------------------------------------------
# ccs profile add
# ---------------------------------------------------------------------------


def test_profile_add_from_preset(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, {"profiles": {"default": {}}})
    _run_cli(["profile", "add", "myollama", "--preset", "ollama"], monkeypatch, tmp_path)

    config = {"profiles": {"default": {}}}
    from claude_swap.cli import cmd_profile_add

    cmd_profile_add(config, "myollama", "ollama")
    captured = capsys.readouterr()
    assert "myollama" in captured.out

    # Verify it was saved to disk
    saved = yaml.safe_load((tmp_path / "config.yaml").read_text())
    assert "myollama" in saved["profiles"]
    assert saved["profiles"]["myollama"]["base_url"] == "http://localhost:11434/v1"


def test_profile_add_default_name_rejected(monkeypatch, tmp_path):
    _write_config(tmp_path, {"profiles": {"default": {}}})
    _run_cli(["profile", "add", "default"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_profile_add

    with pytest.raises(SystemExit) as exc_info:
        cmd_profile_add({"profiles": {"default": {}}}, "default", None)
    assert exc_info.value.code == 1


def test_profile_add_duplicate_rejected(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _run_cli(["profile", "add", "ollama", "--preset", "ollama"], monkeypatch, tmp_path)

    from claude_swap.cli import cmd_profile_add

    with pytest.raises(SystemExit) as exc_info:
        cmd_profile_add(SAMPLE_CONFIG, "ollama", "ollama")
    assert exc_info.value.code == 1


# ---------------------------------------------------------------------------
# ccs profile remove
# ---------------------------------------------------------------------------


def test_profile_remove_nonactive(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")
    _run_cli(["profile", "remove", "ollama"], monkeypatch, tmp_path)

    # Patch isatty to skip confirmation prompt
    with patch("sys.stdin.isatty", return_value=False):
        from claude_swap.cli import cmd_profile_remove

        cmd_profile_remove(SAMPLE_CONFIG, "ollama")
    captured = capsys.readouterr()
    assert "removed" in captured.out

    saved = yaml.safe_load((tmp_path / "config.yaml").read_text())
    assert "ollama" not in saved["profiles"]


def test_profile_remove_active_blocked(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "ollama")
    _run_cli(["profile", "remove", "ollama"], monkeypatch, tmp_path)

    with pytest.raises(SystemExit) as exc_info:
        from claude_swap.cli import cmd_profile_remove

        cmd_profile_remove(SAMPLE_CONFIG, "ollama")
    assert exc_info.value.code == 1


def test_profile_remove_default_blocked(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "openrouter")
    _run_cli(["profile", "remove", "default"], monkeypatch, tmp_path)

    with pytest.raises(SystemExit) as exc_info:
        from claude_swap.cli import cmd_profile_remove

        cmd_profile_remove(SAMPLE_CONFIG, "default")
    assert exc_info.value.code == 1


# ---------------------------------------------------------------------------
# ccs run — one-shot launch (does NOT change active profile)
# ---------------------------------------------------------------------------


def test_run_does_not_change_active(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")

    captured_exec = {}

    def mock_execvpe(path, args, env):
        captured_exec["path"] = path
        captured_exec["env"] = env

    monkeypatch.setattr(os, "execvpe", mock_execvpe)
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    _run_cli(["run", "ollama"], monkeypatch, tmp_path)

    import importlib
    import claude_swap.launcher as launcher_mod

    importlib.reload(launcher_mod)

    from claude_swap.cli import cmd_run
    import argparse

    cmd_run(argparse.Namespace(profile="ollama", claude_args=[]))

    # Active profile must still be "default"
    from claude_swap.state import get_active_profile

    assert get_active_profile() == "default"
    # But env should have ollama's vars
    assert captured_exec["env"]["ANTHROPIC_BASE_URL"] == "http://localhost:11434/v1"


def test_run_nonexistent_profile_exits_1(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _run_cli(["run", "nonexistent"], monkeypatch, tmp_path)

    import argparse
    from claude_swap.cli import cmd_run

    with pytest.raises(SystemExit) as exc_info:
        cmd_run(argparse.Namespace(profile="nonexistent", claude_args=[]))
    assert exc_info.value.code == 1


# ---------------------------------------------------------------------------
# bare ccs (no subcommand) → triggers launch
# ---------------------------------------------------------------------------


def test_bare_ccs_launches_with_active_profile(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "ollama")

    captured_exec = {}

    def mock_execvpe(path, args, env):
        captured_exec["path"] = path
        captured_exec["env"] = env

    monkeypatch.setattr(os, "execvpe", mock_execvpe)
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    _run_cli([], monkeypatch, tmp_path)

    import importlib
    import claude_swap.launcher as launcher_mod

    importlib.reload(launcher_mod)

    from claude_swap.cli import cmd_launch

    cmd_launch([])
    assert captured_exec["env"]["ANTHROPIC_BASE_URL"] == "http://localhost:11434/v1"


def test_bare_ccs_default_profile_no_overrides(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")

    captured_exec = {}

    def mock_execvpe(path, args, env):
        captured_exec["env"] = env

    monkeypatch.setattr(os, "execvpe", mock_execvpe)
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    monkeypatch.delenv("ANTHROPIC_BASE_URL", raising=False)
    _run_cli([], monkeypatch, tmp_path)

    import importlib
    import claude_swap.launcher as launcher_mod

    importlib.reload(launcher_mod)

    from claude_swap.cli import cmd_launch

    cmd_launch([])
    # Default profile should not set ANTHROPIC_BASE_URL
    assert "ANTHROPIC_BASE_URL" not in captured_exec["env"]


def test_bare_ccs_passes_extra_args(monkeypatch, tmp_path):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")

    captured_exec = {}

    def mock_execvpe(path, args, env):
        captured_exec["args"] = args

    monkeypatch.setattr(os, "execvpe", mock_execvpe)
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    _run_cli([], monkeypatch, tmp_path)

    import importlib
    import claude_swap.launcher as launcher_mod

    importlib.reload(launcher_mod)

    from claude_swap.cli import cmd_launch

    cmd_launch(["--debug", "--verbose"])
    assert "--debug" in captured_exec["args"]
    assert "--verbose" in captured_exec["args"]


# ---------------------------------------------------------------------------
# ccs doctor
# ---------------------------------------------------------------------------


def test_doctor_runs_without_error(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    _run_cli(["doctor"], monkeypatch, tmp_path)

    from claude_swap.doctor import run_doctor

    run_doctor()
    captured = capsys.readouterr()
    assert "claude-code-swap" in captured.out
    assert "[OK]" in captured.out or "[WARN]" in captured.out or "[FAIL]" in captured.out


def test_doctor_shows_all_check_categories(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")
    monkeypatch.setenv("CLAUDE_BINARY", "/fake/claude")
    _run_cli(["doctor"], monkeypatch, tmp_path)

    from claude_swap.doctor import run_doctor

    run_doctor()
    captured = capsys.readouterr()
    assert "Claude Code" in captured.out
    assert "config.yaml" in captured.out
    assert "Active profile" in captured.out


# ---------------------------------------------------------------------------
# main() — full argparse routing smoke test
# ---------------------------------------------------------------------------


def test_main_dispatches_status(monkeypatch, tmp_path, capsys):
    """Full path: main() → parse args → cmd_status → output."""
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")
    monkeypatch.setattr(sys, "argv", ["ccs", "status"])
    monkeypatch.setattr("claude_swap.config.CONFIG_DIR", tmp_path)
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", tmp_path / "config.yaml")
    monkeypatch.setattr("claude_swap.state.STATE_FILE", tmp_path / "state.yaml")
    monkeypatch.setattr("claude_swap.state.CONFIG_DIR", tmp_path)

    from claude_swap.cli import main

    main()
    captured = capsys.readouterr()
    assert "Active profile" in captured.out


def test_main_dispatches_profile_list(monkeypatch, tmp_path, capsys):
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")
    monkeypatch.setattr(sys, "argv", ["ccs", "profile", "list"])
    monkeypatch.setattr("claude_swap.config.CONFIG_DIR", tmp_path)
    monkeypatch.setattr("claude_swap.config.CONFIG_FILE", tmp_path / "config.yaml")
    monkeypatch.setattr("claude_swap.state.STATE_FILE", tmp_path / "state.yaml")
    monkeypatch.setattr("claude_swap.state.CONFIG_DIR", tmp_path)

    from claude_swap.cli import main

    main()
    captured = capsys.readouterr()
    assert "ollama" in captured.out
    assert "openrouter" in captured.out


def test_main_use_then_status(monkeypatch, tmp_path, capsys):
    """Verify use + status round-trip through main()."""
    _write_config(tmp_path, SAMPLE_CONFIG)
    _write_state(tmp_path, "default")

    base_patches = {
        "claude_swap.config.CONFIG_DIR": tmp_path,
        "claude_swap.config.CONFIG_FILE": tmp_path / "config.yaml",
        "claude_swap.state.STATE_FILE": tmp_path / "state.yaml",
        "claude_swap.state.CONFIG_DIR": tmp_path,
    }

    for attr, val in base_patches.items():
        monkeypatch.setattr(attr, val)

    from claude_swap.cli import main

    # Step 1: use ollama
    monkeypatch.setattr(sys, "argv", ["ccs", "use", "ollama"])
    main()

    # Step 2: status should now show ollama
    monkeypatch.setattr(sys, "argv", ["ccs", "status"])
    main()
    captured = capsys.readouterr()
    assert "ollama" in captured.out

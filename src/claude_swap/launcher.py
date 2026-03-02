"""Core launch primitive: construct environment and exec claude."""

from __future__ import annotations

import os
import shutil

from .config import build_env_overrides, resolve_profile_env


def find_claude_binary() -> str:
    """
    Find the claude binary.

    Search order:
      1. CLAUDE_BINARY env var (for testing/overrides)
      2. shutil.which("claude")
      3. Raise FileNotFoundError with install hint
    """
    override = os.environ.get("CLAUDE_BINARY")
    if override:
        return override

    binary = shutil.which("claude")
    if binary:
        return binary

    raise FileNotFoundError(
        "Claude Code not found in PATH.\n"
        "Install it with: npm install -g @anthropic-ai/claude-code"
    )


def build_env(resolved_profile: dict) -> dict[str, str]:
    """
    Build the full environment dict for execvpe.

    Starts with os.environ.copy(), then overlays ANTHROPIC_* vars.
    Never mutates os.environ.
    """
    env = os.environ.copy()
    overrides = build_env_overrides(resolved_profile)
    env.update(overrides)
    return env


def launch(profile: dict, extra_args: list[str]) -> None:
    """
    Replace the current process with claude.

    This function never returns on success.
    Raises FileNotFoundError or ValueError on validation failure.
    """
    resolved = resolve_profile_env(profile)
    env = build_env(resolved)
    binary = find_claude_binary()
    argv = [binary] + extra_args
    os.execvpe(binary, argv, env)
    # Unreachable: execvpe replaces the process

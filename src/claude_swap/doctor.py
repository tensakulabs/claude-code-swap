"""System health checks."""

from __future__ import annotations

import os
import re
import shutil
import stat
import sys
from dataclasses import dataclass
from typing import Literal

import yaml

from . import __version__
from .config import CONFIG_DIR, CONFIG_FILE, load_config
from .state import STATE_FILE, get_active_profile

_ENV_REF_PATTERN = re.compile(r"\$\{([^}]+)\}")


@dataclass
class CheckResult:
    name: str
    status: Literal["ok", "warn", "fail"]
    message: str


def check_claude_binary() -> CheckResult:
    override = os.environ.get("CLAUDE_BINARY")
    binary = override or shutil.which("claude")
    if binary:
        return CheckResult("Claude Code", "ok", binary)
    return CheckResult(
        "Claude Code",
        "fail",
        "Not found in PATH. Install: npm install -g @anthropic-ai/claude-code",
    )


def check_config_exists() -> CheckResult:
    if CONFIG_FILE.exists():
        return CheckResult("config.yaml", "ok", str(CONFIG_FILE))
    return CheckResult(
        "config.yaml", "warn", f"Not found at {CONFIG_FILE}. Run 'ccs init'."
    )


def check_config_parseable() -> CheckResult:
    if not CONFIG_FILE.exists():
        return CheckResult(
            "config.yaml (parse)", "warn", "File not found — skipping parse check"
        )
    try:
        data = yaml.safe_load(CONFIG_FILE.read_text()) or {}
        profile_count = len(data.get("profiles", {}) or {})
        return CheckResult(
            "config.yaml (parse)", "ok", f"Valid YAML, {profile_count} profile(s)"
        )
    except yaml.YAMLError as e:
        return CheckResult("config.yaml (parse)", "fail", f"Invalid YAML: {e}")


def check_config_dir_permissions() -> CheckResult:
    if not CONFIG_DIR.exists():
        return CheckResult(
            "~/.claude-code-swap/ permissions", "warn", "Directory not found"
        )
    mode = stat.S_IMODE(CONFIG_DIR.stat().st_mode)
    if mode == 0o700:
        return CheckResult("~/.claude-code-swap/ permissions", "ok", "0700")
    return CheckResult(
        "~/.claude-code-swap/ permissions",
        "warn",
        f"Expected 0700, got {oct(mode)}. Fix: chmod 700 ~/.claude-code-swap/",
    )


def check_active_profile_valid() -> CheckResult:
    active = get_active_profile()
    if not CONFIG_FILE.exists():
        if active == "default":
            return CheckResult("Active profile", "ok", "default (no config)")
        return CheckResult(
            "Active profile", "warn", f"'{active}' (no config file)"
        )
    try:
        config = load_config()
        profiles = config.get("profiles", {}) or {}
        if active == "default" or active in profiles:
            return CheckResult("Active profile", "ok", active)
        return CheckResult(
            "Active profile",
            "fail",
            f"'{active}' not found in config. Run 'ccs use default'.",
        )
    except Exception:
        return CheckResult(
            "Active profile", "warn", f"Could not verify '{active}'"
        )


def check_env_refs_resolvable() -> CheckResult:
    """Check if env var references in the active profile are resolvable."""
    if not CONFIG_FILE.exists():
        return CheckResult("Env var refs (active)", "warn", "No config — skipping")

    active = get_active_profile()
    if active == "default":
        return CheckResult(
            "Env var refs (active)", "ok", "default profile (no refs)"
        )

    try:
        config = load_config()
        profiles = config.get("profiles", {}) or {}
        profile = profiles.get(active, {})
    except Exception:
        return CheckResult(
            "Env var refs (active)", "warn", "Could not load config"
        )

    def find_refs(value, path=""):
        refs = []
        if isinstance(value, str):
            for m in _ENV_REF_PATTERN.finditer(value):
                refs.append((path, m.group(1)))
        elif isinstance(value, dict):
            for k, v in value.items():
                refs.extend(find_refs(v, f"{path}.{k}" if path else str(k)))
        return refs

    refs = find_refs(profile)
    missing = [(p, v) for p, v in refs if v not in os.environ]

    if not missing:
        return CheckResult(
            "Env var refs (active)", "ok", f"All {len(refs)} ref(s) resolved"
        )

    missing_vars = ", ".join(f"${{{v}}}" for _, v in missing)
    return CheckResult(
        "Env var refs (active)",
        "fail",
        f"Unset variable(s) in '{active}': {missing_vars}",
    )


def run_doctor() -> None:
    """Run all health checks and print results."""
    print(f"claude-swap v{__version__}\n")

    checks = [
        check_claude_binary(),
        check_config_exists(),
        check_config_parseable(),
        check_config_dir_permissions(),
        check_active_profile_valid(),
        check_env_refs_resolvable(),
    ]

    for check in checks:
        icon, color = {
            "ok": ("[OK]  ", "\033[32m"),
            "warn": ("[WARN]", "\033[33m"),
            "fail": ("[FAIL]", "\033[31m"),
        }[check.status]

        reset = "\033[0m"
        if not sys.stdout.isatty():
            color = reset = ""

        print(f"{color}{icon}{reset} {check.name}: {check.message}")

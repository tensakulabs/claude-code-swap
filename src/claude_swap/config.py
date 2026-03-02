"""YAML loading, ${VAR} resolution, env var mapping, and security warnings."""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path
from typing import Any

import yaml

CONFIG_DIR = Path.home() / ".claude-swap"
CONFIG_FILE = CONFIG_DIR / "config.yaml"

_KEY_PATTERN = re.compile(r"\b(sk|or)-[A-Za-z0-9]{20,}\b")
_ENV_REF_PATTERN = re.compile(r"\$\{([^}]+)\}")


def ensure_config_dir() -> None:
    """Create ~/.claude-swap/ with mode 0700."""
    CONFIG_DIR.mkdir(mode=0o700, exist_ok=True)


def load_config() -> dict:
    """Load config.yaml. Returns {} if missing or empty."""
    if not CONFIG_FILE.exists():
        return {}
    try:
        content = CONFIG_FILE.read_text()
        data = yaml.safe_load(content) or {}
    except yaml.YAMLError as e:
        print(f"ccs: error: config.yaml is not valid YAML: {e}", file=sys.stderr)
        raise
    return data


def save_config(config: dict, path: Path | None = None) -> None:
    """Atomically write config.yaml with mode 0600."""
    if path is None:
        path = CONFIG_FILE
    ensure_config_dir()
    tmp = path.with_suffix(".yaml.tmp")
    content = yaml.dump(
        config, default_flow_style=False, sort_keys=True, allow_unicode=True
    )
    tmp.write_text(content)
    os.chmod(tmp, 0o600)
    os.replace(tmp, path)


def resolve_env_refs(value: str) -> str:
    """Expand ${VAR} references from os.environ. Raises ValueError on missing var."""

    def replacer(m: re.Match) -> str:
        var = m.group(1)
        if var not in os.environ:
            raise ValueError(f"Environment variable ${{{var}}} is not set")
        return os.environ[var]

    return _ENV_REF_PATTERN.sub(replacer, value)


def _resolve_value(value: Any) -> Any:
    """Recursively resolve ${VAR} refs in strings within any data structure."""
    if isinstance(value, str):
        return resolve_env_refs(value)
    elif isinstance(value, dict):
        return {k: _resolve_value(v) for k, v in value.items()}
    elif isinstance(value, list):
        return [_resolve_value(v) for v in value]
    return value


def resolve_profile_env(profile: dict) -> dict:
    """Walk all string values in a profile dict, expanding ${VAR} references."""
    return _resolve_value(profile)


def build_env_overrides(profile: dict) -> dict[str, str]:
    """Convert a resolved profile dict to ANTHROPIC_* environment variables."""
    overrides: dict[str, str] = {}

    field_map = {
        "base_url": "ANTHROPIC_BASE_URL",
        "auth_token": "ANTHROPIC_AUTH_TOKEN",
        "api_key": "ANTHROPIC_API_KEY",
    }
    model_map = {
        "haiku": "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "sonnet": "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "opus": "ANTHROPIC_DEFAULT_OPUS_MODEL",
    }

    for field, env_var in field_map.items():
        if field in profile and profile[field] is not None:
            overrides[env_var] = str(profile[field])

    models = profile.get("models", {}) or {}
    for model_key, env_var in model_map.items():
        if model_key in models and models[model_key] is not None:
            overrides[env_var] = str(models[model_key])

    # Pass through any extra env vars from profile.env
    extra_env = profile.get("env", {}) or {}
    overrides.update({k: str(v) for k, v in extra_env.items()})

    return overrides


def _scan_values_for_keys(data: Any, path: str = "") -> list[str]:
    """Recursively scan for hardcoded key-like strings. Returns list of offending paths."""
    warnings: list[str] = []
    if isinstance(data, str):
        if _KEY_PATTERN.search(data):
            warnings.append(path)
    elif isinstance(data, dict):
        for k, v in data.items():
            warnings.extend(
                _scan_values_for_keys(v, f"{path}.{k}" if path else str(k))
            )
    elif isinstance(data, list):
        for i, v in enumerate(data):
            warnings.extend(_scan_values_for_keys(v, f"{path}[{i}]"))
    return warnings


def warn_hardcoded_keys(config: dict) -> None:
    """Print a warning if API key-like strings are found directly in config values."""
    profiles = config.get("profiles", {}) or {}
    paths = _scan_values_for_keys(profiles, "profiles")
    if paths:
        if sys.stderr.isatty():
            yellow = "\033[33m"
            reset = "\033[0m"
        else:
            yellow = reset = ""
        print(
            f"{yellow}ccs: warning: Possible API key(s) found directly in config "
            f"at: {', '.join(paths)}\n"
            f"  Use ${{ENV_VAR}} references instead of hardcoding keys.{reset}",
            file=sys.stderr,
        )

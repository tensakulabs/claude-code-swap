"""Active profile state management."""

from __future__ import annotations

import os
import sys
from pathlib import Path

import yaml

from .config import CONFIG_DIR, ensure_config_dir

STATE_FILE = CONFIG_DIR / "state.yaml"


def get_active_profile() -> str:
    """Returns the active profile name, defaulting to 'default' if state is missing/corrupt."""
    if not STATE_FILE.exists():
        return "default"
    try:
        data = yaml.safe_load(STATE_FILE.read_text()) or {}
        return data.get("active", "default") or "default"
    except Exception:
        return "default"


def set_active_profile(name: str) -> None:
    """Atomically write the active profile name to state.yaml."""
    ensure_config_dir()
    tmp = STATE_FILE.with_suffix(".yaml.tmp")
    content = yaml.dump({"active": name}, default_flow_style=False)
    tmp.write_text(content)
    os.chmod(tmp, 0o600)
    os.replace(tmp, STATE_FILE)

"""Profile CRUD operations. All functions return new dicts — never mutate inputs."""

from __future__ import annotations

from .config import load_config, save_config


def list_profiles(config: dict) -> list[str]:
    """Return sorted list of profile names. Always includes 'default'."""
    profiles = config.get("profiles", {}) or {}
    names = set(profiles.keys())
    names.add("default")
    return sorted(names)


def get_profile(config: dict, name: str) -> dict:
    """Return profile dict. Raises ValueError if not found."""
    profiles = config.get("profiles", {}) or {}
    if name == "default":
        return profiles.get("default", {})
    if name not in profiles:
        available = ", ".join(["default"] + sorted(profiles.keys()))
        raise ValueError(
            f"Profile '{name}' not found. Available: {available}"
        )
    return profiles[name]


def add_profile(config: dict, name: str, data: dict) -> dict:
    """Return a new config dict with the profile added/updated. Caller must save."""
    new_config = dict(config)
    profiles = dict(new_config.get("profiles", {}) or {})
    profiles[name] = data
    new_config["profiles"] = profiles
    return new_config


def remove_profile(config: dict, name: str) -> dict:
    """Return a new config dict with the profile removed. Raises on 'default'."""
    if name == "default":
        raise ValueError("Cannot remove the 'default' profile")
    profiles = config.get("profiles", {}) or {}
    if name not in profiles:
        raise ValueError(f"Profile '{name}' not found")
    new_profiles = {k: v for k, v in profiles.items() if k != name}
    return {**config, "profiles": new_profiles}

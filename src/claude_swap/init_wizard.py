"""Interactive first-time setup wizard."""

from __future__ import annotations

import shutil
import sys

from .config import ensure_config_dir, load_config, save_config
from .presets import PRESET_NAMES, get_preset
from .profiles import add_profile
from .state import set_active_profile


def _is_interactive() -> bool:
    return sys.stdin.isatty() and sys.stdout.isatty()


def _prompt(msg: str, default: str = "") -> str:
    """Prompt with optional default value."""
    if default:
        result = input(f"{msg} [{default}]: ").strip()
        return result or default
    return input(f"{msg}: ").strip()


def _confirm(msg: str, default: bool = True) -> bool:
    """Yes/no prompt."""
    suffix = "(Y/n)" if default else "(y/N)"
    try:
        response = input(f"{msg} {suffix}: ").strip().lower()
    except (EOFError, KeyboardInterrupt):
        print()
        return default
    if not response:
        return default
    return response in ("y", "yes")


def run_wizard() -> None:
    """Interactive first-time setup."""
    from .config import CONFIG_FILE

    if not _is_interactive():
        print("ccs: error: init requires an interactive terminal", file=sys.stderr)
        return

    print("Welcome to claude-swap!\n")

    # Check for existing config
    if CONFIG_FILE.exists():
        if not _confirm("Config already exists. Reinitialize?", default=False):
            print("Aborted.")
            return

    # Find claude binary
    binary = shutil.which("claude")
    if binary:
        print(f"Found Claude Code at: {binary}\n")
    else:
        print("Warning: Claude Code not found in PATH.\n")

    config = load_config() if CONFIG_FILE.exists() else {}
    profiles_added: list[str] = []

    while True:
        if not _confirm("Set up a provider profile?"):
            break

        # Profile name
        name = _prompt("Profile name").strip()
        if not name or name == "default":
            print("Invalid profile name. Try again.")
            continue

        # Provider type
        print(f"\nProvider types: {', '.join(PRESET_NAMES)}")
        provider_type = _prompt("Provider type").strip().lower()

        if provider_type not in PRESET_NAMES:
            print(f"Unknown provider type '{provider_type}'. Using 'custom'.")
            provider_type = "custom"

        profile_data: dict = dict(get_preset(provider_type))

        # Fill in required fields based on preset
        if provider_type == "ollama":
            base_url = _prompt("Base URL", default="http://localhost:11434/v1")
            profile_data["base_url"] = base_url
            models = dict(profile_data.get("models", {}))
            models["haiku"] = _prompt("Haiku model", default=models.get("haiku", ""))
            models["sonnet"] = _prompt("Sonnet model", default=models.get("sonnet", ""))
            models["opus"] = _prompt("Opus model", default=models.get("opus", ""))
            profile_data["models"] = {k: v for k, v in models.items() if v}

        elif provider_type in ("openrouter", "gemini", "openai"):
            env_var_hint = {
                "openrouter": "OPENROUTER_API_KEY",
                "gemini": "GEMINI_API_KEY",
                "openai": "OPENAI_API_KEY",
            }[provider_type]
            print(
                f"API key reference (use ${{ENV_VAR}} or press Enter for ${{{env_var_hint}}})"
            )
            key_input = _prompt("API key env var", default=f"${{{env_var_hint}}}")
            profile_data["auth_token"] = key_input

        elif provider_type == "custom":
            profile_data["base_url"] = _prompt("Base URL")
            print("Auth token (use ${ENV_VAR} for env var references)")
            profile_data["auth_token"] = _prompt("Auth token")
            has_models = _confirm("Configure model overrides?")
            if has_models:
                models = {}
                h = _prompt("Haiku model (or Enter to skip)").strip()
                s = _prompt("Sonnet model (or Enter to skip)").strip()
                o = _prompt("Opus model (or Enter to skip)").strip()
                if h:
                    models["haiku"] = h
                if s:
                    models["sonnet"] = s
                if o:
                    models["opus"] = o
                if models:
                    profile_data["models"] = models

        config = add_profile(config, name, profile_data)
        profiles_added.append(name)
        print(f'\nProfile "{name}" saved.')

        if not _confirm("\nAdd another profile?", default=False):
            break

    if not profiles_added and not CONFIG_FILE.exists():
        # Create minimal config with just default
        config = {"profiles": {"default": {}}}

    # Ask for active profile
    all_profiles = ["default"] + profiles_added
    if profiles_added:
        print(f"\nAvailable profiles: {', '.join(all_profiles)}")
        active = _prompt(
            "Set active profile",
            default=profiles_added[0] if profiles_added else "default",
        )
        if active not in all_profiles:
            active = "default"
    else:
        active = "default"

    # Save
    ensure_config_dir()
    save_config(config)
    set_active_profile(active)

    print(f"\nSetup complete! Active profile: {active}")
    print('\nRun "ccs" to launch Claude Code.')
    if profiles_added:
        print('Run "ccs use <profile>" to switch providers.')

"""CLI entry point — argument parsing and command dispatch."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import yaml

from . import __version__
from .config import (
    CONFIG_FILE,
    ensure_config_dir,
    load_config,
    save_config,
    warn_hardcoded_keys,
)
from .doctor import run_doctor
from .init_wizard import run_wizard
from .launcher import launch
from .presets import PRESET_NAMES
from .profiles import add_profile, get_profile, list_profiles, remove_profile
from .state import get_active_profile, set_active_profile
from .tester import test_profile


# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------


def _err(msg: str) -> None:
    if sys.stderr.isatty():
        print(f"\033[31mccs: error:\033[0m {msg}", file=sys.stderr)
    else:
        print(f"ccs: error: {msg}", file=sys.stderr)


def _warn(msg: str) -> None:
    if sys.stderr.isatty():
        print(f"\033[33mccs: warning:\033[0m {msg}", file=sys.stderr)
    else:
        print(f"ccs: warning: {msg}", file=sys.stderr)


def _profile_summary(name: str, profile: dict) -> str:
    """Return a short human-readable description of a profile."""
    if not profile:
        return "Anthropic subscription (Pro/Max)"

    base_url = profile.get("base_url", "")
    models = profile.get("models", {}) or {}

    # Detect provider from base_url
    provider = "Unknown"
    if "ollama" in base_url or "11434" in base_url:
        provider = "Local Ollama"
    elif "openrouter" in base_url:
        provider = "OpenRouter"
    elif "googleapis" in base_url:
        provider = "Google Gemini"
    elif "openai.com" in base_url:
        provider = "OpenAI"
    elif base_url:
        # Use just the hostname
        try:
            from urllib.parse import urlparse

            provider = urlparse(base_url).netloc or base_url
        except Exception:
            provider = base_url

    primary = models.get("sonnet") or models.get("haiku") or ""
    if primary:
        return f"{provider} -> {primary}"
    return provider


def _confirm_reopen() -> bool:
    if not sys.stdin.isatty():
        return False
    try:
        resp = input("Re-open in editor? (y/N): ").strip().lower()
    except (EOFError, KeyboardInterrupt):
        return False
    return resp in ("y", "yes")


# ---------------------------------------------------------------------------
# Command handlers
# ---------------------------------------------------------------------------


def cmd_launch(extra_args: list[str]) -> None:
    """Launch Claude Code with the active profile."""
    config = load_config()
    warn_hardcoded_keys(config)

    active = get_active_profile()
    try:
        profile = get_profile(config, active)
    except ValueError as e:
        _err(str(e))
        sys.exit(1)

    try:
        launch(profile, extra_args)
    except FileNotFoundError as e:
        _err(str(e))
        sys.exit(1)
    except ValueError as e:
        _err(str(e))
        sys.exit(1)


def cmd_use(args: argparse.Namespace) -> None:
    """Set the active profile."""
    config = load_config()
    profiles = config.get("profiles", {}) or {}
    name: str = args.profile

    if name != "default" and name not in profiles:
        available = ", ".join(["default"] + sorted(profiles.keys()))
        _err(f"Profile '{name}' not found. Available: {available}")
        sys.exit(1)

    set_active_profile(name)

    try:
        profile = get_profile(config, name)
    except ValueError:
        profile = {}

    description = _profile_summary(name, profile)
    print(f"Switched to: {name} ({description})")


def cmd_run(args: argparse.Namespace) -> None:
    """Launch with a specific profile (one-shot, does not change active)."""
    config = load_config()
    warn_hardcoded_keys(config)

    try:
        profile = get_profile(config, args.profile)
    except ValueError as e:
        _err(str(e))
        sys.exit(1)

    extra_args: list[str] = list(args.claude_args or [])

    try:
        launch(profile, extra_args)
    except (FileNotFoundError, ValueError) as e:
        _err(str(e))
        sys.exit(1)


def cmd_status(args: argparse.Namespace) -> None:
    """Show current active profile and its config."""
    print(f"claude-swap v{__version__}\n")

    active = get_active_profile()
    config = load_config()

    try:
        profile = get_profile(config, active)
    except ValueError:
        profile = {}

    print(f"Active profile: {active}")

    if not profile:
        print("  (no configuration — Anthropic subscription)")
    else:
        base_url = profile.get("base_url", "")
        models = profile.get("models", {}) or {}
        extra_env = profile.get("env", {}) or {}

        if base_url:
            print(f"  Base URL: {base_url}")
        for role in ("haiku", "sonnet", "opus"):
            model = models.get(role)
            if model:
                print(f"  {role.capitalize():<8} {model}")
        for k, v in extra_env.items():
            print(f"  {k}: {v}")

    print(f'\nRun "ccs use <profile>" to switch. Run "ccs profile list" for all profiles.')


def cmd_profile_list(config: dict) -> None:
    active = get_active_profile()
    profiles_dict = config.get("profiles", {}) or {}
    all_names = list_profiles(config)

    for name in all_names:
        marker = "*" if name == active else " "
        profile = profiles_dict.get(name, {}) if name != "default" else profiles_dict.get("default", {})
        description = _profile_summary(name, profile)
        print(f"  {marker} {name:<20} {description}")


def cmd_profile_show(config: dict, name: str | None) -> None:
    if name is None:
        name = get_active_profile()

    try:
        profile = get_profile(config, name)
    except ValueError as e:
        _err(str(e))
        sys.exit(1)

    print(f"Profile: {name}")
    if not profile:
        print("  (empty — Anthropic subscription, no overrides)")
    else:
        print(yaml.dump(profile, default_flow_style=False).rstrip())


def cmd_profile_add(config: dict, name: str, preset: str | None) -> None:
    """Add a profile from preset or open in editor."""
    if name == "default":
        _err("Cannot add a profile named 'default' — it already exists implicitly")
        sys.exit(1)

    profiles_dict = config.get("profiles", {}) or {}
    if name in profiles_dict:
        _err(f"Profile '{name}' already exists. Use 'ccs profile edit {name}' to modify.")
        sys.exit(1)

    if preset:
        from .presets import get_preset as _get_preset

        data = _get_preset(preset)
        new_config = add_profile(config, name, data)
        save_config(new_config)
        print(f'Profile "{name}" added from preset "{preset}".')
        print(f'Edit it with: ccs profile edit {name}')
    else:
        # Open editor with a template
        template = yaml.dump(
            {
                "base_url": "",
                "auth_token": "${YOUR_API_KEY}",
                "api_key": "",
                "models": {"haiku": "", "sonnet": "", "opus": ""},
            },
            default_flow_style=False,
        )
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".yaml", delete=False, prefix=f"ccs-profile-{name}-"
        ) as f:
            f.write(f"# Profile: {name}\n# Edit and save to add.\n\n{template}")
            tmp_path = Path(f.name)

        editor = os.environ.get("VISUAL") or os.environ.get("EDITOR") or "vi"
        subprocess.run([editor, str(tmp_path)])

        try:
            profile_data = yaml.safe_load(tmp_path.read_text()) or {}
        except Exception as e:
            _err(f"Failed to parse profile: {e}")
            tmp_path.unlink(missing_ok=True)
            sys.exit(1)

        tmp_path.unlink(missing_ok=True)
        new_config = add_profile(config, name, profile_data)
        save_config(new_config)
        print(f'Profile "{name}" added.')


def cmd_profile_edit(config: dict, name: str) -> None:
    """Open a profile in $EDITOR."""
    try:
        profile = get_profile(config, name)
    except ValueError as e:
        _err(str(e))
        sys.exit(1)

    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".yaml", delete=False, prefix=f"ccs-profile-{name}-"
    ) as f:
        f.write(f"# Profile: {name}\n\n")
        f.write(yaml.dump(profile, default_flow_style=False))
        tmp_path = Path(f.name)

    editor = os.environ.get("VISUAL") or os.environ.get("EDITOR") or "vi"

    while True:
        subprocess.run([editor, str(tmp_path)])
        try:
            new_data = yaml.safe_load(tmp_path.read_text()) or {}
            break
        except yaml.YAMLError as e:
            print(f"YAML error: {e}")
            if not _confirm_reopen():
                tmp_path.unlink(missing_ok=True)
                sys.exit(1)

    tmp_path.unlink(missing_ok=True)
    new_config = add_profile(config, name, new_data)
    save_config(new_config)
    print(f'Profile "{name}" updated.')


def cmd_profile_remove(config: dict, name: str) -> None:
    """Remove a profile with confirmation."""
    active = get_active_profile()
    if name == active:
        _err(f"'{name}' is the active profile. Run 'ccs use default' first.")
        sys.exit(1)

    profiles_dict = config.get("profiles", {}) or {}
    if name not in profiles_dict and name != "default":
        _err(f"Profile '{name}' not found")
        sys.exit(1)

    if sys.stdin.isatty():
        try:
            resp = input(f"Remove profile '{name}'? (y/N): ").strip().lower()
        except (EOFError, KeyboardInterrupt):
            print()
            print("Aborted.")
            return
        if resp not in ("y", "yes"):
            print("Aborted.")
            return

    try:
        new_config = remove_profile(config, name)
    except ValueError as e:
        _err(str(e))
        sys.exit(1)

    save_config(new_config)
    print(f'Profile "{name}" removed.')


def cmd_profile(args: argparse.Namespace) -> None:
    """Dispatch profile subcommands."""
    config = load_config()

    profile_command = getattr(args, "profile_command", None)
    if profile_command is None:
        cmd_profile_list(config)
        return

    dispatch = {
        "list": lambda: cmd_profile_list(config),
        "show": lambda: cmd_profile_show(config, getattr(args, "name", None)),
        "add": lambda: cmd_profile_add(
            config, args.name, getattr(args, "preset", None)
        ),
        "edit": lambda: cmd_profile_edit(config, args.name),
        "remove": lambda: cmd_profile_remove(config, args.name),
    }

    handler = dispatch.get(profile_command)
    if handler:
        handler()
    else:
        _err(f"Unknown profile subcommand: {profile_command}")
        sys.exit(1)


def cmd_test(args: argparse.Namespace) -> None:
    """Test provider connectivity."""
    config = load_config()
    name = getattr(args, "profile", None) or get_active_profile()

    try:
        profile = get_profile(config, name)
    except ValueError as e:
        _err(str(e))
        sys.exit(1)

    test_profile(name, profile)


def cmd_config_open(args: argparse.Namespace) -> None:
    """Open config.yaml in $EDITOR."""
    if not CONFIG_FILE.exists():
        print(f"Config file not found at {CONFIG_FILE}. Run 'ccs init' first.")
        return

    editor = os.environ.get("VISUAL") or os.environ.get("EDITOR") or "vi"
    subprocess.run([editor, str(CONFIG_FILE)])


# ---------------------------------------------------------------------------
# Argument parser
# ---------------------------------------------------------------------------


def create_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="ccs",
        description="Claude Code provider profile switcher",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  ccs                         Launch with active profile
  ccs use ollama              Switch to ollama profile
  ccs run openrouter          One-shot launch with openrouter (does not change active)
  ccs status                  Show active profile
  ccs profile list            List all profiles
  ccs profile add mywork      Add a new profile interactively
  ccs test                    Test active profile connectivity
  ccs doctor                  System health check
  ccs init                    First-time setup wizard
        """.strip(),
    )
    parser.add_argument("--version", action="version", version=f"ccs {__version__}")

    subparsers = parser.add_subparsers(dest="command")

    # ccs use <profile>
    p_use = subparsers.add_parser(
        "use", help="Set active profile (persists across sessions)"
    )
    p_use.add_argument("profile", help="Profile name")

    # ccs run <profile> [claude-args...]
    p_run = subparsers.add_parser(
        "run", help="One-shot launch with specific profile (does not change active)"
    )
    p_run.add_argument("profile", help="Profile name")
    p_run.add_argument(
        "claude_args",
        nargs=argparse.REMAINDER,
        help="Additional args passed directly to claude",
    )

    # ccs status
    subparsers.add_parser("status", help="Show active profile and its config")

    # ccs profile
    p_profile = subparsers.add_parser("profile", help="Manage profiles")
    pp = p_profile.add_subparsers(dest="profile_command")
    pp.add_parser("list", help="List all profiles")

    p_add = pp.add_parser("add", help="Add a new profile")
    p_add.add_argument("name", help="Profile name")
    p_add.add_argument(
        "--preset",
        choices=PRESET_NAMES,
        help="Initialize from a provider preset",
    )

    p_edit = pp.add_parser("edit", help="Edit a profile in $EDITOR")
    p_edit.add_argument("name", help="Profile name")

    p_rm = pp.add_parser("remove", help="Remove a profile")
    p_rm.add_argument("name", help="Profile name")

    p_show = pp.add_parser("show", help="Show full profile config")
    p_show.add_argument("name", nargs="?", help="Profile name (default: active)")

    # ccs test [profile]
    p_test = subparsers.add_parser("test", help="Test provider connectivity")
    p_test.add_argument(
        "profile", nargs="?", help="Profile name (default: active)"
    )

    # ccs doctor
    subparsers.add_parser("doctor", help="Run system health checks")

    # ccs init
    subparsers.add_parser("init", help="Interactive first-time setup wizard")

    # ccs config
    subparsers.add_parser("config", help="Open config.yaml in $EDITOR")

    return parser


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> None:
    parser = create_parser()

    # parse_known_args allows bare `ccs` to forward unknown args to claude
    args, unknown = parser.parse_known_args()

    if args.command is None:
        # No subcommand: launch claude with active profile + any extra args
        cmd_launch(unknown)
        return  # unreachable after execvpe

    dispatch = {
        "use": cmd_use,
        "run": cmd_run,
        "status": cmd_status,
        "profile": cmd_profile,
        "test": cmd_test,
        "doctor": lambda _: run_doctor(),
        "init": lambda _: run_wizard(),
        "config": cmd_config_open,
    }

    handler = dispatch.get(args.command)
    if handler is None:
        parser.print_help()
        sys.exit(1)

    try:
        handler(args)
    except KeyboardInterrupt:
        print()
        sys.exit(130)
    except BrokenPipeError:
        sys.exit(0)
    except Exception as e:
        _err(str(e))
        sys.exit(1)

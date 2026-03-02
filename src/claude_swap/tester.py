"""Provider connectivity testing via urllib."""

from __future__ import annotations

import json
import sys
import time
import urllib.error
import urllib.request
from typing import NamedTuple

from .config import resolve_profile_env


class TestResult(NamedTuple):
    ok: bool
    message: str
    elapsed_ms: int | None = None


def _make_request(url: str, token: str | None, timeout: int = 10) -> TestResult:
    """Send a GET request. Returns TestResult."""
    headers: dict[str, str] = {}
    if token:
        headers["Authorization"] = f"Bearer {token}"

    req = urllib.request.Request(url, headers=headers, method="GET")
    start = time.monotonic()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            elapsed = int((time.monotonic() - start) * 1000)
            return TestResult(ok=True, message=f"HTTP {resp.status}", elapsed_ms=elapsed)
    except urllib.error.HTTPError as e:
        elapsed = int((time.monotonic() - start) * 1000)
        # 401/403 = server reachable, auth issue — still connectivity success
        if e.code in (401, 403):
            return TestResult(
                ok=True,
                message=f"HTTP {e.code} (auth issue — server reachable)",
                elapsed_ms=elapsed,
            )
        return TestResult(ok=False, message=f"HTTP {e.code}: {e.reason}", elapsed_ms=elapsed)
    except urllib.error.URLError as e:
        elapsed = int((time.monotonic() - start) * 1000)
        return TestResult(ok=False, message=f"Connection failed: {e.reason}", elapsed_ms=elapsed)
    except OSError as e:
        elapsed = int((time.monotonic() - start) * 1000)
        return TestResult(ok=False, message=f"Error: {e}", elapsed_ms=elapsed)


def test_profile(profile_name: str, profile: dict) -> None:
    """Test connectivity for a profile. Prints results to stdout."""
    print(f'Testing "{profile_name}" profile...')

    try:
        resolved = resolve_profile_env(profile)
    except ValueError as e:
        print(f"  {_red('ERROR')}: {e}")
        return

    base_url = resolved.get("base_url", "").rstrip("/")
    auth_token = resolved.get("auth_token") or resolved.get("api_key") or None
    models = resolved.get("models", {}) or {}

    if not base_url:
        print(f"  {_yellow('SKIP')}: No base_url configured (default profile)")
        return

    # Test base URL reachability via /models endpoint
    models_url = f"{base_url}/models"
    result = _make_request(models_url, auth_token)

    if result.ok:
        print(f"  Base URL:  {base_url}  {_green('reachable')} ({result.elapsed_ms}ms)")
    else:
        print(f"  Base URL:  {base_url}  {_red('unreachable')} — {result.message}")
        print("  Skipping model tests (base URL unreachable)")
        return

    # Test each model with a minimal completion request
    all_ok = True
    for role in ("haiku", "sonnet", "opus"):
        model_id = models.get(role)
        if not model_id:
            continue

        model_url = f"{base_url}/chat/completions"
        test_body = json.dumps(
            {
                "model": model_id,
                "messages": [{"role": "user", "content": "hi"}],
                "max_tokens": 1,
            }
        ).encode()

        headers: dict[str, str] = {"Content-Type": "application/json"}
        if auth_token:
            headers["Authorization"] = f"Bearer {auth_token}"

        req = urllib.request.Request(model_url, data=test_body, headers=headers, method="POST")
        start = time.monotonic()
        try:
            with urllib.request.urlopen(req, timeout=15) as resp:
                elapsed = int((time.monotonic() - start) * 1000)
                print(
                    f"  {role.capitalize():<8} {model_id:<40} {_green('ok')} ({elapsed}ms)"
                )
        except urllib.error.HTTPError as e:
            elapsed = int((time.monotonic() - start) * 1000)
            all_ok = False
            if e.code == 404:
                print(
                    f"  {role.capitalize():<8} {model_id:<40} "
                    f"{_red('404 model not found')}"
                )
            else:
                print(
                    f"  {role.capitalize():<8} {model_id:<40} {_red(f'HTTP {e.code}')}"
                )
        except Exception as e:
            all_ok = False
            print(
                f"  {role.capitalize():<8} {model_id:<40} {_red(f'Error: {e}')}"
            )

    if all_ok:
        print("All models OK.")
    else:
        print(
            f"\n{_yellow('Warning')}: Some models failed. "
            f'Run "ccs profile edit {profile_name}" to fix.'
        )


def _green(s: str) -> str:
    return f"\033[32m{s}\033[0m" if sys.stdout.isatty() else s


def _yellow(s: str) -> str:
    return f"\033[33m{s}\033[0m" if sys.stdout.isatty() else s


def _red(s: str) -> str:
    return f"\033[31m{s}\033[0m" if sys.stdout.isatty() else s

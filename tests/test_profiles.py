"""Tests for profiles.py: CRUD operations."""

from __future__ import annotations

import pytest


SAMPLE_CONFIG = {
    "profiles": {
        "default": {},
        "ollama": {
            "base_url": "http://localhost:11434/v1",
            "auth_token": "ollama",
            "models": {"sonnet": "llama3.1:8b"},
        },
        "openrouter": {
            "base_url": "https://openrouter.ai/api/v1",
            "auth_token": "${OPENROUTER_API_KEY}",
        },
    }
}


# ---------------------------------------------------------------------------
# list_profiles
# ---------------------------------------------------------------------------


def test_list_profiles_sorted():
    from claude_swap.profiles import list_profiles

    result = list_profiles(SAMPLE_CONFIG)
    assert result == sorted(result)


def test_list_profiles_includes_default():
    from claude_swap.profiles import list_profiles

    result = list_profiles({"profiles": {"ollama": {}}})
    assert "default" in result


def test_list_profiles_empty_config():
    from claude_swap.profiles import list_profiles

    result = list_profiles({})
    assert result == ["default"]


# ---------------------------------------------------------------------------
# get_profile
# ---------------------------------------------------------------------------


def test_get_profile_existing():
    from claude_swap.profiles import get_profile

    result = get_profile(SAMPLE_CONFIG, "ollama")
    assert result["base_url"] == "http://localhost:11434/v1"


def test_get_profile_default_returns_empty_when_not_defined():
    from claude_swap.profiles import get_profile

    config = {"profiles": {"ollama": {}}}
    result = get_profile(config, "default")
    assert result == {}


def test_get_profile_default_from_explicit_definition():
    from claude_swap.profiles import get_profile

    config = {"profiles": {"default": {"base_url": "http://x"}}}
    result = get_profile(config, "default")
    assert result["base_url"] == "http://x"


def test_get_profile_not_found_raises():
    from claude_swap.profiles import get_profile

    with pytest.raises(ValueError, match="not found"):
        get_profile(SAMPLE_CONFIG, "nonexistent")


def test_get_profile_error_message_lists_available():
    from claude_swap.profiles import get_profile

    with pytest.raises(ValueError) as exc_info:
        get_profile(SAMPLE_CONFIG, "nonexistent")
    assert "default" in str(exc_info.value)
    assert "ollama" in str(exc_info.value)


# ---------------------------------------------------------------------------
# add_profile
# ---------------------------------------------------------------------------


def test_add_profile_returns_new_config():
    from claude_swap.profiles import add_profile

    original = {"profiles": {"default": {}}}
    new_data = {"base_url": "http://new"}
    result = add_profile(original, "new-profile", new_data)

    assert "new-profile" in result["profiles"]
    # Original not mutated
    assert "new-profile" not in original["profiles"]


def test_add_profile_updates_existing():
    from claude_swap.profiles import add_profile

    config = {"profiles": {"ollama": {"base_url": "old"}}}
    result = add_profile(config, "ollama", {"base_url": "new"})
    assert result["profiles"]["ollama"]["base_url"] == "new"


def test_add_profile_to_empty_config():
    from claude_swap.profiles import add_profile

    result = add_profile({}, "new", {"base_url": "http://x"})
    assert result["profiles"]["new"]["base_url"] == "http://x"


# ---------------------------------------------------------------------------
# remove_profile
# ---------------------------------------------------------------------------


def test_remove_profile_removes():
    from claude_swap.profiles import remove_profile

    result = remove_profile(SAMPLE_CONFIG, "ollama")
    assert "ollama" not in result["profiles"]


def test_remove_profile_does_not_mutate_original():
    from claude_swap.profiles import remove_profile

    result = remove_profile(SAMPLE_CONFIG, "ollama")
    assert "ollama" in SAMPLE_CONFIG["profiles"]


def test_remove_profile_default_raises():
    from claude_swap.profiles import remove_profile

    with pytest.raises(ValueError, match="default"):
        remove_profile(SAMPLE_CONFIG, "default")


def test_remove_profile_not_found_raises():
    from claude_swap.profiles import remove_profile

    with pytest.raises(ValueError, match="not found"):
        remove_profile(SAMPLE_CONFIG, "nonexistent")

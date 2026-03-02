# claude-swap (`ccs`)

> Switch Claude Code provider profiles with one command.

Claude Code is locked to Anthropic models by default. `ccs` (Claude Code Switch) lets you manage multiple provider profiles — Ollama, OpenRouter, Gemini, OpenAI, or any OpenAI-compatible endpoint — and launch Claude Code with the right environment variables in a single command.

**No proxy layer. No added latency. Just environment variables.**

---

## Install

```bash
pip install claude-swap
# or
pipx install claude-swap
```

Single-line install:

```bash
curl -sSL https://raw.githubusercontent.com/tensakulabs/claude-swap/main/install.sh | bash
```

---

## Quick Start

```bash
# First-time setup
ccs init

# Switch to Ollama
ccs use ollama

# Launch Claude Code (uses active profile automatically)
ccs

# Switch back to Anthropic subscription
ccs use default
ccs
```

---

## Commands

```bash
# Launch (uses active profile)
ccs                             # Launch with active profile
ccs run ollama                  # One-shot with specific profile (doesn't change active)

# Switching
ccs use ollama                  # Set ollama as active (persists across sessions)
ccs use default                 # Back to Anthropic subscription
ccs status                      # Show active profile and its config

# Profile management
ccs profile list                # List all profiles (* = active)
ccs profile add mywork          # Add a new profile interactively
ccs profile add mywork --preset openrouter   # Add from a preset
ccs profile edit mywork         # Edit in $EDITOR
ccs profile remove mywork       # Delete a profile
ccs profile show mywork         # Show full profile config

# Diagnostics
ccs test                        # Test active profile connectivity
ccs test ollama                 # Test specific profile
ccs doctor                      # System health check
ccs config                      # Open config.yaml in $EDITOR
ccs --version
```

---

## How It Works

`ccs` stores your profiles in `~/.claude-swap/config.yaml`. When you run `ccs`, it:

1. Reads the active profile from `~/.claude-swap/state.yaml`
2. Resolves any `${ENV_VAR}` references from your shell environment
3. Constructs the appropriate `ANTHROPIC_*` environment variables
4. Calls `os.execvpe("claude", ...)` — **replacing its own process** with Claude Code

No proxy. No subprocess. No added latency. After exec, `ccs` is completely out of the picture.

---

## Configuration

**`~/.claude-swap/config.yaml`** — your profiles:

```yaml
profiles:
  default: {}  # No overrides — uses Anthropic subscription as-is

  ollama:
    base_url: http://localhost:11434/v1
    auth_token: ollama
    models:
      haiku: llama3.2:3b
      sonnet: llama3.1:8b
      opus: llama3.1:70b

  openrouter:
    base_url: https://openrouter.ai/api/v1
    auth_token: ${OPENROUTER_API_KEY}   # resolved from your shell env at launch
    api_key: ""
    models:
      haiku: qwen/qwen3-4b:free
      sonnet: qwen/qwen3-coder:free
      opus: deepseek/deepseek-r1-0528:free

  gemini:
    base_url: https://generativelanguage.googleapis.com/v1beta/openai
    auth_token: ${GEMINI_API_KEY}
    api_key: ""
    models:
      haiku: gemini-2.0-flash
      sonnet: gemini-2.5-flash
      opus: gemini-2.5-pro
```

**`~/.claude-swap/state.yaml`** — active profile (written by `ccs use`):

```yaml
active: ollama
```

State is separated from config so `ccs use` never touches your hand-edited profiles.

---

## Environment Variable Mapping

| Profile field | Environment variable |
|---|---|
| `base_url` | `ANTHROPIC_BASE_URL` |
| `auth_token` | `ANTHROPIC_AUTH_TOKEN` |
| `api_key` | `ANTHROPIC_API_KEY` |
| `models.haiku` | `ANTHROPIC_DEFAULT_HAIKU_MODEL` |
| `models.sonnet` | `ANTHROPIC_DEFAULT_SONNET_MODEL` |
| `models.opus` | `ANTHROPIC_DEFAULT_OPUS_MODEL` |
| `env.*` | Passed through as-is |

The `default` profile sets none of these — Claude Code runs with your existing environment untouched.

---

## Provider Presets

`ccs profile add myprofile --preset openrouter` pre-fills defaults:

| Preset | Base URL |
|---|---|
| `ollama` | `http://localhost:11434/v1` |
| `openrouter` | `https://openrouter.ai/api/v1` |
| `gemini` | `https://generativelanguage.googleapis.com/v1beta/openai` |
| `openai` | `https://api.openai.com/v1` |
| `custom` | (you provide) |

---

## Security

- **Never hardcode API keys** — always use `${ENV_VAR}` references. `ccs` warns you if it detects a key-like string in your config.
- `~/.claude-swap/` is created with `0700`, files with `0600`.
- `ccs` makes **no network calls** during normal launch — only during `ccs test`.
- State file contains only a profile name, no secrets.

---

## Requirements

- Python 3.10+
- PyYAML 6.0+
- Claude Code (`npm install -g @anthropic-ai/claude-code`)

---

## License

MIT — see [LICENSE](LICENSE)

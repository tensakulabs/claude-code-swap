# claude-code-swap (ccs) — Developer Notes

## Known Limitations / Future Work

### OAuth-based providers (Codex, GitHub Copilot)

Some providers use OAuth rather than raw API keys. These cannot be used with ccs directly today.

#### OpenAI Codex CLI
- Auth: OAuth via `~/.codex/auth.json` (`access_token`, `refresh_token`, `account_id`)
- Endpoint: `https://chatgpt.com/backend-api/codex/responses` (NOT `api.openai.com/v1`)
- Protocol: OpenAI Responses API format + requires `ChatGPT-Account-ID` header
- **Blocker**: endpoint is not Anthropic-compatible; Claude Code cannot speak to it
- **Path forward**: a local proxy that reads `~/.codex/auth.json`, handles token refresh, translates to Anthropic format, and exposes `localhost:XXXX/anthropic` — ccs's `custom` preset could then point there

#### GitHub Copilot
- Auth: OAuth via VS Code extension or `gh auth`
- Endpoint: VS Code LM API (`localhost:3000/v1` via copilot-proxy pattern)
- **Blocker**: same as Codex — not Anthropic-compatible without a proxy layer
- **Path forward**: same local proxy pattern; copilot-proxy in OpenClaw is the reference implementation

#### General proxy pattern
If a local proxy exists that exposes an Anthropic-compatible endpoint using OAuth internally, ccs already supports it via the `custom` preset — just point `base_url` at the proxy's localhost address. No ccs code changes needed; the proxy is the missing piece.

#### Providers that DO work with OAuth tokens
MiniMax and Alibaba Cloud expose Anthropic-compatible endpoints that accept OAuth Bearer tokens directly (same as API keys). If a user obtains an OAuth token for these providers, it can be set as `auth_token` in ccs with no changes needed.

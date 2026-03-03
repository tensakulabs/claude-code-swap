#!/usr/bin/env node
"use strict";

/**
 * Node.js shim for claude-code-swap (ccs).
 *
 * This npm package is a convenience wrapper. The actual CLI is a native Rust binary.
 * Install order of precedence:
 *   1. `ccs` binary already on PATH (installed via cargo/homebrew/download) — exec directly
 *   2. Error with installation instructions
 */

const { spawnSync } = require("child_process");

const args = process.argv.slice(2);

// Try ccs binary on PATH
function findOnPath(name) {
  try {
    const result = spawnSync(process.platform === "win32" ? "where" : "which", [name], {
      encoding: "utf8",
      stdio: "pipe",
    });
    if (result.status === 0 && result.stdout.trim()) {
      return result.stdout.trim().split("\n")[0].trim();
    }
  } catch (_) {}
  return null;
}

const ccsBin = findOnPath("ccs");
if (ccsBin) {
  const result = spawnSync(ccsBin, args, { stdio: "inherit" });
  process.exit(result.status ?? 1);
}

// Nothing found — helpful error
console.error(`
ccs: claude-code-swap binary not found.

Install the native binary using one of:

  cargo install claude-code-swap
  # or
  brew install tensakulabs/tap/ccs

Then re-run your command.
`.trim());
process.exit(1);

#!/usr/bin/env node
"use strict";

/**
 * Node.js shim for claude-code-swap (ccs).
 *
 * This npm package is a convenience wrapper. The actual CLI is a Python package.
 * Install order of precedence:
 *   1. `ccs` binary already on PATH (installed via pip/pipx) — exec directly
 *   2. `python3 -m claude_swap.cli` — if the Python package is importable
 *   3. Error with installation instructions
 */

const { execFileSync, spawnSync } = require("child_process");
const { existsSync } = require("fs");
const path = require("path");

const args = process.argv.slice(2);

// 1. Try ccs binary on PATH (fastest path — pip/pipx install already done)
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

// 2. Try python3 -m claude_swap.cli
const pythons = ["python3", "python"];
for (const py of pythons) {
  const pyBin = findOnPath(py);
  if (!pyBin) continue;

  // Check that claude_swap is importable
  const check = spawnSync(pyBin, ["-c", "import claude_swap"], { stdio: "pipe" });
  if (check.status !== 0) continue;

  const result = spawnSync(pyBin, ["-m", "claude_swap.cli", ...args], { stdio: "inherit" });
  process.exit(result.status ?? 1);
}

// 3. Nothing found — helpful error
console.error(`
ccs: claude-code-swap Python package not found.

This npm package is a shim that requires the Python package to be installed:

  pip install claude-code-swap
  # or
  pipx install claude-code-swap

Requires Python 3.10+. After installing, re-run your command.
`.trim());
process.exit(1);

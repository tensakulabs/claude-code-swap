#!/usr/bin/env node
"use strict";

/**
 * Entry-point shim for @tensakulabs/ccs.
 *
 * Resolution order:
 *   1. Bundled binary (bin/ccs) — installed by postinstall from GitHub releases
 *   2. `ccs` on PATH — installed via brew or manual download
 *   3. Helpful error
 */

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const args = process.argv.slice(2);

// 1. Bundled binary placed by install.js postinstall
const bundled = path.join(__dirname, process.platform === "win32" ? "ccs.exe" : "ccs");
if (fs.existsSync(bundled)) {
  const result = spawnSync(bundled, args, { stdio: "inherit" });
  process.exit(result.status ?? 1);
}

// 2. ccs already on PATH (brew, manual download)
function findOnPath(name) {
  try {
    const result = spawnSync(
      process.platform === "win32" ? "where" : "which",
      [name],
      { encoding: "utf8", stdio: "pipe" }
    );
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

// 3. Nothing found
console.error(
  "ccs: binary not found.\n" +
  "\n" +
  "Re-run the postinstall script to re-download it:\n" +
  "  node $(npm root -g)/@tensakulabs/ccs/install.js\n" +
  "\n" +
  "Or download the binary directly from GitHub releases:\n" +
  "  https://github.com/tensakulabs/claude-code-swap/releases/latest\n" +
  "\n" +
  "macOS users can also install via Homebrew:\n" +
  "  brew install tensakulabs/tap/ccs"
);
process.exit(1);

#!/usr/bin/env node
"use strict";

/**
 * Postinstall script for @tensakulabs/ccs.
 *
 * Downloads the pre-compiled native binary from the matching GitHub release
 * and places it at bin/ccs (or bin/ccs.exe on Windows) so that bin/ccs.js
 * can exec it directly without requiring a separate install step.
 *
 * Failures are non-fatal: if the download fails, bin/ccs.js falls back to
 * looking for `ccs` on PATH (e.g. installed via brew or manual download).
 */

const https = require("https");
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const REPO = "tensakulabs/claude-code-swap";
const BIN_DIR = path.join(__dirname, "bin");
const BIN_NAME = process.platform === "win32" ? "ccs.exe" : "ccs";
const BIN_PATH = path.join(BIN_DIR, BIN_NAME);

function getArchiveName() {
  const { platform, arch } = process;
  if (platform === "darwin" && arch === "arm64") return "ccs-macos-aarch64.tar.gz";
  if (platform === "darwin" && arch === "x64")  return "ccs-macos-x86_64.tar.gz";
  if (platform === "linux"  && arch === "x64")  return "ccs-linux-x86_64.tar.gz";
  return null;
}

function get(url, cb) {
  https.get(url, { headers: { "User-Agent": "ccs-installer" } }, (res) => {
    if (res.statusCode === 301 || res.statusCode === 302) {
      get(res.headers.location, cb);
      return;
    }
    cb(null, res);
  }).on("error", cb);
}

function download(url, dest, cb) {
  get(url, (err, res) => {
    if (err) return cb(err);
    if (res.statusCode !== 200) return cb(new Error(`HTTP ${res.statusCode} for ${url}`));
    const out = fs.createWriteStream(dest);
    res.pipe(out);
    out.on("finish", () => cb(null));
    out.on("error", cb);
  });
}

function printNextSteps() {
  process.stdout.write("\n");
  process.stdout.write("  Quick start:\n");
  process.stdout.write("    ccs init          Set up your first provider profile\n");
  process.stdout.write("    ccs use <name>    Switch to a different profile\n");
  process.stdout.write("    ccs               Launch Claude Code with the active profile\n");
  process.stdout.write("\n");
  process.stdout.write("  Docs: https://github.com/tensakulabs/claude-code-swap\n");
  process.stdout.write("\n");
}

function main() {
  const archive = getArchiveName();
  if (!archive) {
    process.stdout.write(
      `ccs: no pre-built binary available for ${process.platform}/${process.arch}.\n`
    );
    process.stdout.write("  Download manually from: https://github.com/tensakulabs/claude-code-swap/releases/latest\n");
    if (process.platform === "darwin") {
      process.stdout.write("  Or: brew install tensakulabs/tap/ccs\n");
    }
    printNextSteps();
    return;
  }

  if (fs.existsSync(BIN_PATH)) {
    printNextSteps();
    return; // Already present (e.g. re-install of same version).
  }

  const version = require("./package.json").version;
  const url = `https://github.com/${REPO}/releases/download/v${version}/${archive}`;
  const tmp = path.join(BIN_DIR, "_ccs_install.tar.gz");

  process.stdout.write(`ccs: downloading binary (v${version}, ${process.platform}/${process.arch})...`);

  download(url, tmp, (err) => {
    if (err) {
      try { fs.unlinkSync(tmp); } catch (_) {}
      process.stdout.write(`\nccs: download failed: ${err.message}\n`);
      process.stdout.write("  Download manually from: https://github.com/tensakulabs/claude-code-swap/releases/latest\n");
      if (process.platform === "darwin") {
        process.stdout.write("  Or: brew install tensakulabs/tap/ccs\n");
      }
      return;
    }

    try {
      execFileSync("tar", ["xzf", tmp, "-C", BIN_DIR, "ccs"], { stdio: "pipe" });
      fs.chmodSync(BIN_PATH, 0o755);
      fs.unlinkSync(tmp);
      process.stdout.write(" done.\n");
      printNextSteps();
    } catch (e) {
      try { fs.unlinkSync(tmp); } catch (_) {}
      process.stdout.write(`\nccs: extraction failed: ${e.message}\n`);
      process.stdout.write("  Download manually from: https://github.com/tensakulabs/claude-code-swap/releases/latest\n");
      if (process.platform === "darwin") {
        process.stdout.write("  Or: brew install tensakulabs/tap/ccs\n");
      }
    }
  });
}

main();

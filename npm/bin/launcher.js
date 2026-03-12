#!/usr/bin/env node
"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

function createLauncher(overrides = {}) {
  const fsMod = overrides.fs || fs;
  const pathMod = overrides.path || path;
  const osMod = overrides.os || os;
  const processMod = overrides.process || process;
  const consoleMod = overrides.console || console;
  const spawnSyncFn = overrides.spawnSync || childProcess.spawnSync;
  const execFileSyncFn = overrides.execFileSync || childProcess.execFileSync;
  const packageJson = overrides.packageJson || require("../package.json");
  const installScriptPath = overrides.installScriptPath
    || pathMod.join(__dirname, "..", "scripts", "install.js");

  function resolveInstallDir() {
    if (overrides.installDir) {
      return overrides.installDir;
    }
    if (processMod.env.TOKENIZOR_HOME) {
      return pathMod.join(processMod.env.TOKENIZOR_HOME, "bin");
    }
    return pathMod.join(osMod.homedir(), ".tokenizor", "bin");
  }

  const ext = processMod.platform === "win32" ? ".exe" : "";
  const installDir = resolveInstallDir();
  const binPath = pathMod.join(installDir, "tokenizor-mcp" + ext);
  const pendingPath = pathMod.join(installDir, "tokenizor-mcp.pending" + ext);

  function relayInstallerOutput(output) {
    if (!output) {
      return;
    }
    const text = typeof output === "string" ? output : String(output);
    for (const line of text.split(/\r?\n/)) {
      if (line) {
        consoleMod.error(line);
      }
    }
  }

  function getInstalledVersion() {
    try {
      const output = execFileSyncFn(binPath, ["--version"], {
        encoding: "utf8",
        timeout: 5000,
        env: processMod.env,
      }).trim();
      const match = output.match(/(\d+\.\d+\.\d+)/);
      return match ? match[1] : null;
    } catch {
      return null;
    }
  }

  function applyPendingUpdate() {
    if (!fsMod.existsSync(pendingPath)) {
      return false;
    }

    try {
      fsMod.renameSync(pendingPath, binPath);
      consoleMod.error("tokenizor-mcp: applied pending update.");
      return true;
    } catch {
      return false;
    }
  }

  function runInstaller() {
    try {
      const stdout = execFileSyncFn(processMod.execPath, [installScriptPath], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
        env: processMod.env,
      });
      relayInstallerOutput(stdout);
    } catch (error) {
      relayInstallerOutput(error.stdout);
      relayInstallerOutput(error.stderr);
      throw error;
    }
  }

  function detectClients() {
    const clients = [];
    const home = osMod.homedir();
    if (fsMod.existsSync(pathMod.join(home, ".claude"))) clients.push("claude");
    if (fsMod.existsSync(pathMod.join(home, ".codex"))) clients.push("codex");
    if (clients.length === 0 || clients.length >= 2) return "all";
    return clients[0];
  }

  function runAutoInit() {
    const client = detectClients();
    consoleMod.error(`tokenizor-mcp: auto-configuring for ${client}...`);
    try {
      const output = execFileSyncFn(binPath, ["init", "--client", client], {
        encoding: "utf8",
        timeout: 15000,
        env: processMod.env,
      });
      relayInstallerOutput(output);
    } catch (error) {
      consoleMod.error(
        `tokenizor-mcp: auto-init warning: ${error.message}`
      );
    }
  }

  function ensureInstalledBinary() {
    const pendingApplied = applyPendingUpdate();

    const expectedVersion = packageJson.version;
    const hasBinary = fsMod.existsSync(binPath);
    const installedVersion = hasBinary ? getInstalledVersion() : null;

    if (installedVersion === expectedVersion) {
      // If a pending update was just applied, run init to ensure config matches
      if (pendingApplied) {
        runAutoInit();
      }
      return;
    }

    if (!hasBinary) {
      consoleMod.error("tokenizor-mcp binary not found. Running install...");
    } else {
      consoleMod.error(
        `tokenizor-mcp binary version ${installedVersion || "unknown"} does not match wrapper version ${expectedVersion}. Running install...`
      );
    }

    runInstaller();
    applyPendingUpdate();

    if (!fsMod.existsSync(binPath)) {
      throw new Error("tokenizor-mcp binary is still missing after install.");
    }
  }

  function main(args) {
    ensureInstalledBinary();
    const result = spawnSyncFn(binPath, args, {
      stdio: "inherit",
      env: processMod.env,
    });
    return result.status ?? 1;
  }

  return {
    applyPendingUpdate,
    detectClients,
    ensureInstalledBinary,
    getInstalledVersion,
    getBinaryPath: () => binPath,
    getPendingPath: () => pendingPath,
    main,
    runAutoInit,
  };
}

module.exports = { createLauncher };

if (require.main === module) {
  process.exit(createLauncher().main(process.argv.slice(2)));
}

#!/usr/bin/env node
"use strict";

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");
const https = require("https");
const http = require("http");

const REPO = "special-place-administrator/tokenizor_agentic_mcp";

// Binary lives outside node_modules so npm can update the JS wrapper
// even while the MCP server holds a lock on the running .exe (Windows).
const INSTALL_DIR = path.join(os.homedir(), ".tokenizor", "bin");

function getPlatformArtifact() {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === "win32" && arch === "x64") return "tokenizor-mcp-windows-x64.exe";
  if (platform === "darwin" && arch === "arm64") return "tokenizor-mcp-macos-arm64";
  if (platform === "darwin" && arch === "x64") return "tokenizor-mcp-macos-x64";
  if (platform === "linux" && arch === "x64") return "tokenizor-mcp-linux-x64";

  console.error(`Unsupported platform: ${platform}-${arch}`);
  console.error("Build from source: https://github.com/" + REPO);
  process.exit(1);
}

function getVersion() {
  const pkg = require("../package.json");
  return pkg.version;
}

function getBinaryPath() {
  const ext = process.platform === "win32" ? ".exe" : "";
  return path.join(INSTALL_DIR, "tokenizor-mcp" + ext);
}

function getPendingPath() {
  const ext = process.platform === "win32" ? ".exe" : "";
  return path.join(INSTALL_DIR, "tokenizor-mcp.pending" + ext);
}

function download(url) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith("https") ? https : http;
    client.get(url, { headers: { "User-Agent": "tokenizor-mcp" } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return download(res.headers.location).then(resolve).catch(reject);
      }
      if (res.statusCode !== 200) {
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      const chunks = [];
      res.on("data", (chunk) => chunks.push(chunk));
      res.on("end", () => resolve(Buffer.concat(chunks)));
      res.on("error", reject);
    }).on("error", reject);
  });
}

function getInstalledVersion(binPath) {
  try {
    const output = execSync(`"${binPath}" --version`, {
      encoding: "utf8",
      timeout: 5000,
    }).trim();
    const match = output.match(/(\d+\.\d+\.\d+)/);
    return match ? match[1] : null;
  } catch {
    return null;
  }
}

async function main() {
  const binPath = getBinaryPath();
  const pendingPath = getPendingPath();
  const version = getVersion();

  // Skip only if binary exists AND matches the expected version
  if (fs.existsSync(binPath)) {
    const installed = getInstalledVersion(binPath);
    if (installed === version) {
      try { fs.unlinkSync(pendingPath); } catch {}
      console.log(`tokenizor-mcp v${version} already installed at ${binPath}`);
      return;
    }
    console.log(
      `tokenizor-mcp v${installed || "unknown"} found, updating to v${version}...`
    );
  }

  const artifact = getPlatformArtifact();
  const url = `https://github.com/${REPO}/releases/download/v${version}/${artifact}`;

  console.log(`Downloading tokenizor-mcp v${version} for ${process.platform}-${process.arch}...`);
  console.log(`  ${url}`);

  try {
    const data = await download(url);
    fs.mkdirSync(INSTALL_DIR, { recursive: true });

    try {
      fs.writeFileSync(binPath, data);
      fs.chmodSync(binPath, 0o755);
      try { fs.unlinkSync(pendingPath); } catch {}
      console.log(`Installed: ${binPath}`);
    } catch (writeErr) {
      // Binary locked by running MCP server — stage for next launch
      if (writeErr.code === "EPERM" || writeErr.code === "EBUSY") {
        fs.writeFileSync(pendingPath, data);
        fs.chmodSync(pendingPath, 0o755);
        console.log(`Binary is locked (MCP server running). Staged update at: ${pendingPath}`);
        console.log(`Update will apply automatically on next launch.`);
      } else {
        throw writeErr;
      }
    }
  } catch (err) {
    console.error(`Failed to download binary: ${err.message}`);
    console.error("");
    console.error("You can build from source instead:");
    console.error("  git clone https://github.com/" + REPO);
    console.error("  cd tokenizor_agentic_mcp");
    console.error("  cargo build --release");
    process.exit(1);
  }
}

main();

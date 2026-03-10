#!/usr/bin/env node
"use strict";

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const https = require("https");
const http = require("http");

const REPO = "special-place-administrator/tokenizor_agentic_mcp";
const BIN_DIR = path.join(__dirname, "..", "bin");

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
  const artifact = getPlatformArtifact();
  const ext = process.platform === "win32" ? ".exe" : "";
  return path.join(BIN_DIR, "tokenizor-mcp" + ext);
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

async function main() {
  const binPath = getBinaryPath();

  // Skip if binary already exists
  if (fs.existsSync(binPath)) {
    console.log("tokenizor-mcp binary already installed.");
    return;
  }

  const version = getVersion();
  const artifact = getPlatformArtifact();
  const url = `https://github.com/${REPO}/releases/download/v${version}/${artifact}`;

  console.log(`Downloading tokenizor-mcp v${version} for ${process.platform}-${process.arch}...`);
  console.log(`  ${url}`);

  try {
    const data = await download(url);

    fs.mkdirSync(BIN_DIR, { recursive: true });
    fs.writeFileSync(binPath, data);
    fs.chmodSync(binPath, 0o755);

    console.log(`Installed: ${binPath}`);
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

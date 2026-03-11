#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");

const ext = process.platform === "win32" ? ".exe" : "";
const binPath = path.join(__dirname, "tokenizor-mcp" + ext);
const pendingPath = path.join(__dirname, "tokenizor-mcp.pending" + ext);

// Apply pending update if one was staged (binary was locked during npm update)
if (fs.existsSync(pendingPath)) {
  try {
    fs.renameSync(pendingPath, binPath);
    console.error("tokenizor-mcp: applied pending update.");
  } catch {
    // Still locked — will try again next launch
  }
}

if (!fs.existsSync(binPath)) {
  console.error("tokenizor-mcp binary not found. Running install...");
  try {
    require("child_process").execFileSync(
      process.execPath,
      [path.join(__dirname, "..", "scripts", "install.js")],
      { stdio: "inherit" }
    );
  } catch {
    process.exit(1);
  }
}

const args = process.argv.slice(2);

try {
  const result = require("child_process").spawnSync(binPath, args, {
    stdio: "inherit",
    env: process.env,
  });
  process.exit(result.status ?? 1);
} catch (err) {
  console.error(err.message);
  process.exit(1);
}

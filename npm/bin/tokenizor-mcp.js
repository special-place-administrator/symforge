#!/usr/bin/env node
"use strict";

const { execFileSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const ext = process.platform === "win32" ? ".exe" : "";
const binPath = path.join(__dirname, "tokenizor-mcp" + ext);

if (!fs.existsSync(binPath)) {
  console.error("tokenizor-mcp binary not found. Running install...");
  try {
    execFileSync(process.execPath, [path.join(__dirname, "..", "scripts", "install.js")], {
      stdio: "inherit",
    });
  } catch {
    process.exit(1);
  }
}

const args = process.argv.slice(2);
if (args.length === 0) args.push("run");

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

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const os = require("node:os");
const { spawnSync } = require("node:child_process");
const winPath = path.win32;

const { createLauncher } = require("../bin/launcher.js");

function createFs({
  binPath,
  pendingPath,
  versionPath,
  pendingVersionPath,
  hasBinary = true,
  hasPending = false,
  installedVersion = null,
  pendingVersion = null,
}) {
  const renames = [];
  const writes = [];
  const files = new Map();

  if (hasBinary) {
    files.set(binPath, "binary");
  }
  if (hasPending) {
    files.set(pendingPath, "pending-binary");
  }
  if (installedVersion) {
    files.set(versionPath, `${installedVersion}\n`);
  }
  if (pendingVersion) {
    files.set(pendingVersionPath, `${pendingVersion}\n`);
  }

  return {
    renames,
    writes,
    existsSync(target) {
      return files.has(target);
    },
    readFileSync(target, encoding) {
      if (!files.has(target)) {
        const error = new Error(`ENOENT: ${target}`);
        error.code = "ENOENT";
        throw error;
      }
      const value = files.get(target);
      return encoding ? String(value) : Buffer.from(String(value));
    },
    writeFileSync(target, data) {
      const normalized = Buffer.isBuffer(data) ? data.toString("utf8") : String(data);
      writes.push({ target, data: normalized });
      files.set(target, normalized);
    },
    renameSync(from, to) {
      renames.push({ from, to });
      if (!files.has(from)) {
        throw new Error("unexpected rename");
      }
      if (
        (from === pendingPath && to === binPath)
        || (from === pendingVersionPath && to === versionPath)
      ) {
        files.set(to, files.get(from));
        files.delete(from);
        return;
      }
      throw new Error("unexpected rename");
    },
  };
}

function createLauncherForTest({
  fsOverrides,
  execFileSync,
  spawnSync,
  installDir,
  packageVersion = "0.3.12",
  env = {},
}) {
  const logs = [];
  const errors = [];
  const processMock = {
    platform: "win32",
    arch: "x64",
    env,
    execPath: "C:\\node\\node.exe",
  };
  const consoleMock = {
    log(message) {
      logs.push(message);
    },
    error(message) {
      errors.push(message);
    },
  };

  const launcher = createLauncher({
    fs: fsOverrides,
    path: winPath,
    os: { homedir: () => "C:\\Users\\tester" },
    process: processMock,
    console: consoleMock,
    packageJson: { version: packageVersion },
    installDir,
    execFileSync,
    spawnSync,
  });

  return { launcher, logs, errors };
}

test("launcher runs installer when installed binary version lags wrapper version", () => {
  const installDir = winPath.join("C:\\Users\\tester", ".symforge", "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const versionPath = winPath.join(installDir, "symforge.version");
  const pendingVersionPath = winPath.join(installDir, "symforge.pending.version");
  const fsOverrides = createFs({ binPath, pendingPath, versionPath, pendingVersionPath });
  const execCalls = [];
  let versionCalls = 0;

  const { launcher, errors } = createLauncherForTest({
    fsOverrides,
    installDir,
    execFileSync(command, args) {
      execCalls.push({ command, args });
      if (command === binPath) {
        versionCalls += 1;
        return versionCalls === 1 ? "symforge 0.3.11" : "symforge 0.3.12";
      }
      return "";
    },
    spawnSync() {
      return { status: 0 };
    },
  });

  const status = launcher.main(["--version"]);

  assert.equal(status, 0);
  assert.equal(execCalls[1].command, "C:\\node\\node.exe");
  assert.match(errors.join("\n"), /does not match wrapper version 0.3.12/);
});

test("launcher applies pending update before checking installed version", () => {
  const installDir = winPath.join("C:\\Users\\tester", ".symforge", "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const versionPath = winPath.join(installDir, "symforge.version");
  const pendingVersionPath = winPath.join(installDir, "symforge.pending.version");
  const fsOverrides = createFs({
    binPath,
    pendingPath,
    versionPath,
    pendingVersionPath,
    hasBinary: true,
    hasPending: true,
  });

  const { launcher, errors } = createLauncherForTest({
    fsOverrides,
    installDir,
    execFileSync(command) {
      if (command === binPath) {
        return "symforge 0.3.12";
      }
      throw new Error("installer should not run");
    },
    spawnSync() {
      return { status: 0 };
    },
  });

  const status = launcher.main([]);

  assert.equal(status, 0);
  assert.equal(fsOverrides.renames.length, 1);
  assert.match(errors.join("\n"), /applied pending update/);
});

test("launcher honors SYMFORGE_HOME for binary resolution", () => {
  const installDir = winPath.join("D:\\sandbox", "symforge-home", "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const versionPath = winPath.join(installDir, "symforge.version");
  const pendingVersionPath = winPath.join(installDir, "symforge.pending.version");
  const fsOverrides = createFs({
    binPath,
    pendingPath,
    versionPath,
    pendingVersionPath,
    hasBinary: false,
    hasPending: false,
  });

  const { launcher } = createLauncherForTest({
    fsOverrides,
    installDir: undefined,
    env: { SYMFORGE_HOME: winPath.join("D:\\sandbox", "symforge-home") },
    execFileSync() {
      return "";
    },
    spawnSync() {
      return { status: 0 };
    },
  });

  assert.equal(launcher.getBinaryPath(), binPath);
  assert.equal(launcher.getPendingPath(), pendingPath);
});

test("launcher relays installer stdout to stderr so MCP stdout stays clean", () => {
  const installDir = winPath.join("C:\\Users\\tester", ".symforge", "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const versionPath = winPath.join(installDir, "symforge.version");
  const pendingVersionPath = winPath.join(installDir, "symforge.pending.version");
  const fsOverrides = createFs({ binPath, pendingPath, versionPath, pendingVersionPath });

  const { launcher, logs, errors } = createLauncherForTest({
    fsOverrides,
    installDir,
    execFileSync(command) {
      if (command === binPath) {
        return "symforge 0.3.11";
      }
      return "Downloading symforge v0.3.12...\nInstalled: C:\\Users\\tester\\.symforge\\bin\\symforge.exe\n";
    },
    spawnSync() {
      return { status: 0 };
    },
  });

  const status = launcher.main([]);

  assert.equal(status, 0);
  assert.equal(logs.length, 0);
  assert.match(errors.join("\n"), /Downloading symforge v0.3.12/);
  assert.match(errors.join("\n"), /Installed:/);
});

test("launcher trusts recorded version metadata when probing the binary is unavailable", () => {
  const installDir = winPath.join("C:\\Users\\tester", ".symforge", "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const versionPath = winPath.join(installDir, "symforge.version");
  const pendingVersionPath = winPath.join(installDir, "symforge.pending.version");
  const fsOverrides = createFs({
    binPath,
    pendingPath,
    versionPath,
    pendingVersionPath,
    installedVersion: "0.3.12",
  });
  const execCalls = [];

  const { launcher, errors } = createLauncherForTest({
    fsOverrides,
    installDir,
    execFileSync(command) {
      execCalls.push(command);
      throw Object.assign(new Error(`spawnSync ${command} EPERM`), { code: "EPERM" });
    },
    spawnSync() {
      return { status: 0 };
    },
  });

  const status = launcher.main(["--version"]);

  assert.equal(status, 0);
  assert.deepEqual(execCalls, []);
  assert.equal(errors.length, 0);
});

test("launcher promotes pending version metadata alongside a pending binary", () => {
  const installDir = winPath.join("C:\\Users\\tester", ".symforge", "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const versionPath = winPath.join(installDir, "symforge.version");
  const pendingVersionPath = winPath.join(installDir, "symforge.pending.version");
  const fsOverrides = createFs({
    binPath,
    pendingPath,
    versionPath,
    pendingVersionPath,
    hasPending: true,
    pendingVersion: "0.3.12",
  });

  const { launcher, errors } = createLauncherForTest({
    fsOverrides,
    installDir,
    execFileSync(command) {
      throw Object.assign(new Error(`spawnSync ${command} EPERM`), { code: "EPERM" });
    },
    spawnSync() {
      return { status: 0 };
    },
  });

  const status = launcher.main([]);

  assert.equal(status, 0);
  assert.deepEqual(
    fsOverrides.renames,
    [
      { from: pendingPath, to: binPath },
      { from: pendingVersionPath, to: versionPath },
    ]
  );
  assert.match(errors.join("\n"), /applied pending update/);
});

test(
  "launcher smoke-tests symforge --version end-to-end via a stub binary",
  {
    skip:
      process.platform === "win32"
        ? "Windows cannot execute shebang stubs via CreateProcess as symforge.exe; follow-up: add a pre-built PE or .cmd/launcher-hook path"
        : false,
  },
  (t) => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "symforge-smoke-"));
    t.after(() => {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    });

    const binDir = path.join(tmpDir, "bin");
    fs.mkdirSync(binDir, { recursive: true });

    const stubPath = path.join(binDir, "symforge");
    const stubBody =
      `#!${process.execPath}\n` +
      `"use strict";\n` +
      `const args = process.argv.slice(2);\n` +
      `process.stdout.write("symforge 0.0.0-test " + JSON.stringify(args) + "\\n");\n` +
      `process.exit(0);\n`;
    fs.writeFileSync(stubPath, stubBody);
    fs.chmodSync(stubPath, 0o755);

    const pkg = require("../package.json");
    fs.writeFileSync(
      path.join(binDir, "symforge.version"),
      `${pkg.version}\n`,
    );

    const symforgeEntry = path.join(__dirname, "..", "bin", "symforge.js");
    const result = spawnSync(
      process.execPath,
      [symforgeEntry, "--version"],
      {
        env: { ...process.env, SYMFORGE_HOME: tmpDir },
        encoding: "utf8",
      },
    );

    assert.equal(
      result.status,
      0,
      `launcher exited ${result.status}; stderr=${result.stderr}; stdout=${result.stdout}`,
    );
    assert.match(
      result.stdout,
      /symforge/,
      `stdout missing 'symforge': ${JSON.stringify(result.stdout)}`,
    );
    assert.match(
      result.stdout,
      /\["--version"\]/,
      `--version not forwarded to stub: ${JSON.stringify(result.stdout)}`,
    );
  },
);

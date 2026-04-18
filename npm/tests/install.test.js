const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const winPath = path.win32;

const { createInstaller } = require("../scripts/install.js");

function createFs({
  binPath,
  pendingPath,
  versionPath,
  pendingVersionPath,
  installDir,
  existingPaths = [],
  binFailuresBeforeSuccess = 0,
  hasBinary = true,
  hasPending = false,
  installedVersion = null,
  pendingVersion = null,
}) {
  let binFailuresRemaining = binFailuresBeforeSuccess;
  const writes = [];
  const chmods = [];
  const mkdirs = [];
  const unlinks = [];
  const files = new Map();

  if (hasBinary) {
    files.set(binPath, "existing-binary");
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
  for (const existingPath of existingPaths) {
    files.set(existingPath, "");
  }

  return {
    writes,
    chmods,
    mkdirs,
    unlinks,
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
      if (target === binPath && binFailuresRemaining > 0) {
        binFailuresRemaining -= 1;
        const error = new Error("binary is busy");
        error.code = "EPERM";
        throw error;
      }
      files.set(target, normalized);
    },
    chmodSync(target, mode) {
      chmods.push({ target, mode });
    },
    mkdirSync(target, options) {
      mkdirs.push({ target, options });
      assert.equal(target, installDir);
    },
    unlinkSync(target) {
      unlinks.push(target);
      files.delete(target);
      assert.ok(target === pendingPath || target === pendingVersionPath);
    },
  };
}

function createInstallerForTest({
  fsOverrides,
  execFileSync,
  sleep,
  installDir,
  env = {},
  download,
  packageVersion = "0.3.9",
}) {
  const logs = [];
  const errors = [];
  const processMock = {
    platform: "win32",
    arch: "x64",
    env,
    exit(code) {
      throw new Error(`unexpected exit ${code}`);
    },
  };
  const consoleMock = {
    log(message) {
      logs.push(message);
    },
    error(message) {
      errors.push(message);
    },
  };

  const installer = createInstaller({
    fs: fsOverrides,
    path: winPath,
    os: { homedir: () => "C:\\Users\\tester" },
    process: processMock,
    console: consoleMock,
    packageJson: { version: packageVersion },
    installDir,
    execSync: () => "symforge 0.3.8",
    execFileSync,
    sleep: sleep || (async () => {}),
    download: download || (async () => Buffer.from("new-binary")),
  });

  return { installer, logs, errors };
}

test("locked Windows binary is replaced after stopping running SymForge processes", async () => {
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
    installDir,
    existingPaths: [
      "C:\\Users\\tester\\.claude",
      "C:\\Users\\tester\\.codex",
      "C:\\Users\\tester\\.gemini",
    ],
    binFailuresBeforeSuccess: 1,
  });
  const execCalls = [];
  const { installer, logs } = createInstallerForTest({
    fsOverrides,
    installDir,
    execFileSync(command, args, options) {
      execCalls.push({ command, args, options });
      return "[101,202]";
    },
  });

  await installer.main();

  const powershellCalls = execCalls.filter((c) => c.command === "powershell.exe");
  const nonPsCalls = execCalls.filter((c) => c.command !== "powershell.exe");
  const versionCalls = nonPsCalls.filter((c) => c.args.includes("--version"));
  const initCalls = nonPsCalls.filter((c) => c.args.some((a) => /init/.test(a)));
  // stopAllRunningProcesses + stopRunningWindowsProcesses (EPERM fallback)
  assert.equal(powershellCalls.length, 2);
  // getInstalledVersion calls the binary with --version
  assert.equal(versionCalls.length, 1);
  // runAutoInit calls the installed binary once per detected home-scoped client
  assert.equal(initCalls.length, 3);
  assert.deepEqual(
    initCalls.map((call) => call.args.slice(-1)[0]).sort(),
    ["claude", "codex", "gemini"]
  );
  assert.deepEqual(
    initCalls.map((call) => call.options.cwd),
    ["C:\\Users\\tester", "C:\\Users\\tester", "C:\\Users\\tester"]
  );
  assert.equal(
    fsOverrides.writes.filter((entry) => entry.target === binPath).length,
    2
  );
  assert.equal(
    fsOverrides.writes.some((entry) => entry.target === pendingPath),
    false
  );
  assert.match(logs.join("\n"), /Stopped.*running SymForge process/);
  assert.match(logs.join("\n"), /Installed:/);
  assert.match(logs.join("\n"), /Auto-configuring for detected client\(s\): claude, codex, gemini/);
  assert.match(logs.join("\n"), /Kilo Code is workspace-local/);
});

test("installer pre-stop targets all symforge processes instead of daemon-only command lines", async () => {
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
    installDir,
  });
  const execCalls = [];
  const { installer } = createInstallerForTest({
    fsOverrides,
    installDir,
    execFileSync(command, args, options) {
      execCalls.push({ command, args, options });
      if (command === "powershell.exe") {
        return "[]";
      }
      if (args.includes("--version")) {
        return "symforge 0.3.8";
      }
      return "";
    },
  });

  await installer.main();

  const firstPowerShellCall = execCalls.find((call) => call.command === "powershell.exe");
  assert.ok(firstPowerShellCall, "expected a pre-install PowerShell stop call");
  const commandText = firstPowerShellCall.args.join(" ");
  assert.match(commandText, /ExecutablePath/);
  assert.doesNotMatch(commandText, /\\bdaemon\\b/);
});

test("installer stages a pending binary when the executable is still locked after stopping processes", async () => {
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
    installDir,
    binFailuresBeforeSuccess: 9,
  });
  const { installer, logs } = createInstallerForTest({
    fsOverrides,
    installDir,
    execFileSync() {
      return "[404]";
    },
  });

  await installer.main();

  assert.equal(
    fsOverrides.writes.some((entry) => entry.target === pendingPath),
    true
  );
  assert.match(logs.join("\n"), /Staged update at:/);
  assert.match(logs.join("\n"), /Update will apply automatically on next launch/);
});

test("installer honors SYMFORGE_HOME for binary resolution", () => {
  const symforgeHome = winPath.join("D:\\sandbox", "symforge-home");
  const installDir = winPath.join(symforgeHome, "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const versionPath = winPath.join(installDir, "symforge.version");
  const pendingVersionPath = winPath.join(installDir, "symforge.pending.version");
  const fsOverrides = createFs({
    binPath,
    pendingPath,
    versionPath,
    pendingVersionPath,
    installDir,
  });

  const { installer } = createInstallerForTest({
    fsOverrides,
    installDir: undefined,
    env: { SYMFORGE_HOME: symforgeHome },
    execFileSync() {
      return "[]";
    },
  });

  assert.equal(installer.getBinaryPath(), binPath);
  assert.equal(installer.getPendingPath(), pendingPath);
});

test("installer records version metadata next to the installed binary", async () => {
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
    installDir,
    hasBinary: false,
  });
  const { installer } = createInstallerForTest({
    fsOverrides,
    installDir,
    execFileSync() {
      return "[]";
    },
  });

  await installer.main();

  assert.equal(
    fsOverrides.writes.some((entry) => entry.target === versionPath && entry.data.includes("0.3.9")),
    true
  );
  assert.equal(
    fsOverrides.writes.some((entry) => entry.target === pendingVersionPath),
    false
  );
});

test("installer uses the symforge repo slug in release downloads and fallback instructions", async () => {
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
    installDir,
    hasBinary: false,
  });
  let downloadUrl = null;
  const { installer, errors } = createInstallerForTest({
    fsOverrides,
    installDir,
    execFileSync() {
      return "[]";
    },
    download: async (url) => {
      downloadUrl = url;
      throw new Error("download failed");
    },
  });

  await assert.rejects(() => installer.main(), /unexpected exit 1/);

  assert.match(downloadUrl, /github\.com\/special-place-administrator\/symforge\/releases\/download/);
  assert.match(errors.join("\n"), /git clone https:\/\/github\.com\/special-place-administrator\/symforge/);
  assert.match(errors.join("\n"), /cd symforge/);
});

test("installer skips download when installed binary version matches package version", async () => {
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
    installDir,
    hasBinary: true,
    installedVersion: "0.3.9",
  });
  const execCalls = [];
  let downloadCalls = 0;
  const { installer, logs } = createInstallerForTest({
    fsOverrides,
    installDir,
    execFileSync(command, args) {
      execCalls.push({ command, args });
      return "[]";
    },
    download: async () => {
      downloadCalls += 1;
      return Buffer.from("should-not-be-called");
    },
    packageVersion: "0.3.9",
  });

  await installer.main();

  assert.equal(downloadCalls, 0, "installer should not download when version matches");
  assert.equal(
    fsOverrides.writes.filter((entry) => entry.target === binPath).length,
    0,
    "installer should not overwrite binPath on skip"
  );
  assert.equal(
    fsOverrides.writes.filter((entry) => entry.target === versionPath).length,
    0,
    "installer should not rewrite version metadata on skip"
  );
  // Skip path must NOT invoke stopAllRunningProcesses (no PowerShell calls).
  assert.equal(
    execCalls.filter((call) => call.command === "powershell.exe").length,
    0,
    "installer should not attempt to stop processes on skip"
  );
  assert.match(logs.join("\n"), /symforge v0\.3\.9 already installed/);
});

test("installer re-downloads when pre-existing binary cannot report its version", async () => {
  // Dummy/corrupted/unknown file at the target path: no version metadata and
  // the binary can't be executed to self-report. Policy: re-download.
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
    installDir,
    hasBinary: true,
    installedVersion: null,
  });
  let downloadCalls = 0;
  const { installer, logs } = createInstallerForTest({
    fsOverrides,
    installDir,
    execFileSync(command, args) {
      if (args && args.includes("--version")) {
        const err = new Error("dummy file is not executable");
        err.code = "ENOEXEC";
        throw err;
      }
      return "[]";
    },
    download: async () => {
      downloadCalls += 1;
      return Buffer.from("fresh-binary");
    },
    packageVersion: "0.3.9",
  });

  await installer.main();

  assert.equal(downloadCalls, 1, "installer should re-download when version probe fails");
  assert.equal(
    fsOverrides.writes.some(
      (entry) => entry.target === binPath && entry.data === "fresh-binary"
    ),
    true,
    "installer should overwrite the dummy file with freshly downloaded data"
  );
  assert.equal(
    fsOverrides.writes.some(
      (entry) => entry.target === versionPath && entry.data.includes("0.3.9")
    ),
    true,
    "installer should record the fresh version metadata"
  );
  assert.match(logs.join("\n"), /updating to v0\.3\.9/);
});

test("installer re-downloads when version metadata mismatches package version", async () => {
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
    installDir,
    hasBinary: true,
    installedVersion: "0.3.5",
  });
  let downloadCalls = 0;
  const { installer, logs } = createInstallerForTest({
    fsOverrides,
    installDir,
    execFileSync() {
      return "[]";
    },
    download: async () => {
      downloadCalls += 1;
      return Buffer.from("upgraded-binary");
    },
    packageVersion: "0.3.9",
  });

  await installer.main();

  assert.equal(downloadCalls, 1, "installer should download the upgrade");
  assert.equal(
    fsOverrides.writes.some(
      (entry) => entry.target === binPath && entry.data === "upgraded-binary"
    ),
    true
  );
  assert.match(logs.join("\n"), /symforge v0\.3\.5 found, updating to v0\.3\.9/);
});

test("installer skips auto-init when no home-scoped clients are detected", async () => {
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
    installDir,
    hasBinary: false,
  });
  const execCalls = [];
  const { installer, logs } = createInstallerForTest({
    fsOverrides,
    installDir,
    env: {},
    execFileSync(command, args) {
      execCalls.push({ command, args });
      return "[]";
    },
  });

  await installer.main();

  const initCalls = execCalls.filter((call) => call.args && call.args.includes("init"));
  assert.equal(initCalls.length, 0);
  assert.match(logs.join("\n"), /Auto-configuring skipped: no home-scoped clients detected/);
  assert.match(logs.join("\n"), /Kilo Code is workspace-local/);
});

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const winPath = path.win32;

const { createInstaller } = require("../scripts/install.js");

function createFs({ binPath, pendingPath, installDir, binFailuresBeforeSuccess = 0 }) {
  let binFailuresRemaining = binFailuresBeforeSuccess;
  const writes = [];
  const chmods = [];
  const mkdirs = [];
  const unlinks = [];

  return {
    writes,
    chmods,
    mkdirs,
    unlinks,
    existsSync(target) {
      return target === binPath;
    },
    writeFileSync(target, data) {
      writes.push({ target, data: Buffer.from(data).toString("utf8") });
      if (target === binPath && binFailuresRemaining > 0) {
        binFailuresRemaining -= 1;
        const error = new Error("binary is busy");
        error.code = "EPERM";
        throw error;
      }
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
      assert.equal(target, pendingPath);
    },
  };
}

function createInstallerForTest({ fsOverrides, execFileSync, sleep, installDir, env = {} }) {
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
    packageJson: { version: "0.3.9" },
    installDir,
    execSync: () => "symforge 0.3.8",
    execFileSync,
    sleep: sleep || (async () => {}),
    download: async () => Buffer.from("new-binary"),
  });

  return { installer, logs, errors };
}

test("locked Windows binary is replaced after stopping running SymForge processes", async () => {
  const installDir = winPath.join("C:\\Users\\tester", ".symforge", "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const fsOverrides = createFs({
    binPath,
    pendingPath,
    installDir,
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
  // runAutoInit calls the installed binary
  assert.equal(initCalls.length, 1);
  assert.match(initCalls[0].args.join(" "), /init/);
  assert.equal(
    fsOverrides.writes.filter((entry) => entry.target === binPath).length,
    2
  );
  assert.equal(
    fsOverrides.writes.some((entry) => entry.target === pendingPath),
    false
  );
  assert.match(logs.join("\n"), /Stopped.*symforge daemon process/);
  assert.match(logs.join("\n"), /Installed:/);
  assert.match(logs.join("\n"), /Auto-configuring/);
});

test("installer stages a pending binary when the executable is still locked after stopping processes", async () => {
  const installDir = winPath.join("C:\\Users\\tester", ".symforge", "bin");
  const binPath = winPath.join(installDir, "symforge.exe");
  const pendingPath = winPath.join(installDir, "symforge.pending.exe");
  const fsOverrides = createFs({
    binPath,
    pendingPath,
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
  const fsOverrides = createFs({ binPath, pendingPath, installDir });

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

import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import net from "node:net";

const cwd = process.cwd();
const task = process.argv[2] || "build:desktop";
const desktopDevHost = "127.0.0.1";
const desktopDevPort = 3005;
const candidates = [
  cwd,
  resolve(cwd, "apps"),
  resolve(cwd, "..", "apps"),
  resolve(cwd, "..", "..", "apps"),
  resolve(cwd, ".."),
  resolve(cwd, "..", ".."),
];

function hasFrontendPackage(dir) {
  return existsSync(resolve(dir, "package.json"));
}

function hasBuiltFrontendDist(dir) {
  return existsSync(resolve(dir, "out", "index.html"));
}

function canConnect(host, port, timeoutMs = 1000) {
  return new Promise((resolvePromise) => {
    const socket = new net.Socket();
    let settled = false;

    const finish = (result) => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      resolvePromise(result);
    };

    socket.setTimeout(timeoutMs);
    socket.once("connect", () => finish(true));
    socket.once("timeout", () => finish(false));
    socket.once("error", () => finish(false));
    socket.connect(port, host);
  });
}

async function hasReusableDesktopDevServer() {
  const reachable = await canConnect(desktopDevHost, desktopDevPort);
  if (!reachable) {
    return false;
  }

  try {
    const response = await fetch(`http://${desktopDevHost}:${desktopDevPort}`, {
      signal: AbortSignal.timeout(1500),
    });
    return response.ok;
  } catch {
    return false;
  }
}

function resolvePnpmCommand() {
  const baseArgs = ["--dir", frontendDir, "run", task];
  const nodeBinDir = dirname(process.execPath);
  const windowsCandidates = [
    { command: resolve(nodeBinDir, "pnpm.cmd"), args: baseArgs },
    { command: resolve(nodeBinDir, "corepack.cmd"), args: ["pnpm", ...baseArgs] },
    { command: "pnpm.cmd", args: baseArgs },
    { command: "corepack.cmd", args: ["pnpm", ...baseArgs] },
  ];
  const defaultCandidates = [
    { command: "pnpm", args: baseArgs },
    { command: "corepack", args: ["pnpm", ...baseArgs] },
  ];

  const candidates = process.platform === "win32" ? windowsCandidates : defaultCandidates;
  return candidates.find((candidate) => !candidate.command.includes(":") || existsSync(candidate.command)) ?? candidates[0];
}

const frontendDir = candidates.find(hasFrontendPackage);
if (!frontendDir) {
  console.error(`前端项目目录不存在，当前工作目录: ${cwd}`);
  process.exit(1);
}

if (task === "build:desktop" && hasBuiltFrontendDist(frontendDir)) {
  console.log(`前端产物已存在，跳过重复构建: ${resolve(frontendDir, "out", "index.html")}`);
  process.exit(0);
}

if (task === "dev:desktop" && (await hasReusableDesktopDevServer())) {
  console.log(`检测到现有前端开发服务，直接复用: http://${desktopDevHost}:${desktopDevPort}`);
  process.exit(0);
}

const packageManager = resolvePnpmCommand();
console.log(`执行前端任务: ${packageManager.command} ${packageManager.args.join(" ")}`);
const needsShell = process.platform === "win32" && /\.cmd$/i.test(packageManager.command);
const result = spawnSync(packageManager.command, packageManager.args, {
  stdio: "inherit",
  shell: needsShell,
});

if (result.error) {
  console.error(`前端构建启动失败: ${result.error.message}`);
  process.exit(1);
}

if (typeof result.status === "number" && result.status !== 0) {
  process.exit(result.status);
}

process.exit(0);

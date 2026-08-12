#!/usr/bin/env node
// Builds binaries/bgutil-pot-server.zip, the PO-token provider server the
// Windows bundle ships as a resource.
//
// The zip is ~11 MB of third-party build output and is deliberately kept out
// of git (see src-tauri/binaries/README.md), so CI has to produce it or the
// Windows build fails with "resource path ... doesn't exist". There is no
// prebuilt asset to download: the server is TypeScript that has to be
// compiled, so this follows the same recipe the README gives a human.

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync, cpSync, renameSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

const projectRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const outputZip = resolve(projectRoot, "src-tauri", "binaries", "bgutil-pot-server.zip");

// Pinned: the zip has to match the yt-dlp plugin checked in beside it.
const REPO = process.env.BGUTIL_REPO || "https://github.com/Brainicism/bgutil-ytdlp-pot-provider";
const TAG = process.env.BGUTIL_TAG || "1.3.1";

if (process.platform !== "win32") {
  console.log("[INFO] pot-server zip is a Windows-only resource; nothing to do.");
  process.exit(0);
}

if (existsSync(outputZip) && !process.argv.includes("--force")) {
  console.log(`[INFO] ${outputZip} already exists; leaving it alone.`);
  process.exit(0);
}

function run(command, args, cwd) {
  // npm and npx are .cmd shims on Windows and need a shell to resolve;
  // git and powershell do not, and passing their arguments unshelled keeps
  // the paths below safe from anything the shell would try to interpret.
  const needsShell = command === "npm" || command === "npx";
  const result = spawnSync(command, args, { cwd, stdio: "inherit", shell: needsShell });
  if (result.status !== 0) {
    console.error(`[ERROR] ${command} ${args.join(" ")} failed (${result.status})`);
    process.exit(1);
  }
}

const workDir = join(tmpdir(), `bgutil-pot-${process.pid}`);
rmSync(workDir, { recursive: true, force: true });
mkdirSync(workDir, { recursive: true });

console.log(`[INFO] Cloning ${REPO} at ${TAG}`);
run("git", ["clone", "--depth", "1", "--branch", TAG, REPO, "src"], workDir);

const serverDir = join(workDir, "src", "server");
if (!existsSync(serverDir)) {
  console.error(`[ERROR] ${TAG} has no server/ directory`);
  process.exit(1);
}

// canvas has no ARM64 prebuild and the server does not need it, so it would
// fail the build on windows-11-arm for nothing.
const packageJsonPath = join(serverDir, "package.json");
const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
if (packageJson.dependencies?.canvas) {
  delete packageJson.dependencies.canvas;
  writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`, "utf8");
  console.log("[INFO] Dropped the canvas dependency");
}

run("npm", ["install", "--omit=optional", "--no-audit", "--no-fund"], serverDir);
run("npm", ["install", "-D", "typescript", "--no-audit", "--no-fund"], serverDir);
run("npx", ["tsc"], serverDir);
run("npm", ["prune", "--omit=dev", "--no-audit", "--no-fund"], serverDir);

if (!existsSync(join(serverDir, "build", "main.js"))) {
  console.error("[ERROR] build/main.js missing after compile");
  process.exit(1);
}

// The zip's root must hold exactly build/, node_modules/ and package.json,
// so stage those three and let .NET zip the staging directory: it needs no
// external tool, and Compress-Archive is slow over five thousand files.
const stageDir = join(workDir, "stage");
mkdirSync(stageDir, { recursive: true });
renameSync(join(serverDir, "build"), join(stageDir, "build"));
renameSync(join(serverDir, "node_modules"), join(stageDir, "node_modules"));
cpSync(packageJsonPath, join(stageDir, "package.json"));

mkdirSync(dirname(outputZip), { recursive: true });
rmSync(outputZip, { force: true });
run(
  "powershell",
  [
    "-NoProfile",
    "-Command",
    `Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::CreateFromDirectory('${stageDir.replace(/'/g, "''")}', '${outputZip.replace(/'/g, "''")}')`,
  ],
  workDir,
);

rmSync(workDir, { recursive: true, force: true });
const megabytes = (readFileSync(outputZip).length / 1024 / 1024).toFixed(1);
console.log(`[INFO] Built ${outputZip} (${megabytes} MB)`);

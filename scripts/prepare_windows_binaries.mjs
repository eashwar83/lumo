#!/usr/bin/env node
// Puts the large runtime binaries the Windows bundle lists as resources into
// src-tauri/binaries, then checks that every declared resource actually
// exists.
//
// Both files are deliberately kept out of git (see
// src-tauri/binaries/README.md), so a fresh checkout has neither and the
// build fails with "resource path ... doesn't exist" — one file per attempt,
// because tauri reports only the first it misses. The check at the end
// reports all of them at once instead.

import {
  createWriteStream,
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

const projectRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const tauriDir = resolve(projectRoot, "src-tauri");
const binariesDir = resolve(tauriDir, "binaries");
const force = process.argv.includes("--force");

const YTDLP_URL =
  process.env.YTDLP_URL ||
  "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
// Pinned: the server has to match the yt-dlp plugin checked in beside it.
const BGUTIL_REPO =
  process.env.BGUTIL_REPO || "https://github.com/Brainicism/bgutil-ytdlp-pot-provider";
const BGUTIL_TAG = process.env.BGUTIL_TAG || "1.3.1";

if (process.platform !== "win32") {
  console.log("[INFO] These are Windows-only resources; nothing to do.");
  process.exit(0);
}

function run(command, args, cwd) {
  // npm and npx are .cmd shims on Windows and need a shell to resolve;
  // git and powershell do not.
  const needsShell = command === "npm" || command === "npx";
  const result = spawnSync(command, args, { cwd, stdio: "inherit", shell: needsShell });
  if (result.status !== 0) {
    console.error(`[ERROR] ${command} ${args.join(" ")} failed (${result.status})`);
    process.exit(1);
  }
}

function megabytes(path) {
  return (statSync(path).size / 1024 / 1024).toFixed(1);
}

async function ensureYtDlp() {
  const target = join(binariesDir, "yt-dlp.exe");
  if (existsSync(target) && !force) {
    console.log(`[INFO] yt-dlp.exe already present (${megabytes(target)} MB)`);
    return;
  }
  mkdirSync(binariesDir, { recursive: true });
  const partial = `${target}.part`;
  // One flaky connection should not cost a nine-minute build.
  const attempts = 3;
  for (let attempt = 1; ; attempt += 1) {
    try {
      console.log(`[INFO] Downloading ${YTDLP_URL} (attempt ${attempt}/${attempts})`);
      const response = await fetch(YTDLP_URL, { redirect: "follow" });
      if (!response.ok || !response.body) {
        throw new Error(`${response.status} ${response.statusText}`);
      }
      await pipeline(Readable.fromWeb(response.body), createWriteStream(partial));
      break;
    } catch (error) {
      rmSync(partial, { force: true });
      if (attempt === attempts) {
        console.error(`[ERROR] yt-dlp download failed: ${error.message ?? error}`);
        process.exit(1);
      }
      console.warn(`[WARN] ${error.message ?? error}; retrying`);
      await new Promise((done) => setTimeout(done, attempt * 3000));
    }
  }
  // Rename only once it is whole, so an interrupted download can never be
  // mistaken for a good binary on the next run.
  rmSync(target, { force: true });
  renameSync(partial, target);
  console.log(`[INFO] Downloaded yt-dlp.exe (${megabytes(target)} MB)`);
}

function ensurePotServer() {
  const target = join(binariesDir, "bgutil-pot-server.zip");
  if (existsSync(target) && !force) {
    console.log(`[INFO] bgutil-pot-server.zip already present (${megabytes(target)} MB)`);
    return;
  }

  const workDir = join(tmpdir(), `bgutil-pot-${process.pid}`);
  rmSync(workDir, { recursive: true, force: true });
  mkdirSync(workDir, { recursive: true });

  console.log(`[INFO] Cloning ${BGUTIL_REPO} at ${BGUTIL_TAG}`);
  run("git", ["clone", "--depth", "1", "--branch", BGUTIL_TAG, BGUTIL_REPO, "src"], workDir);

  const serverDir = join(workDir, "src", "server");
  if (!existsSync(serverDir)) {
    console.error(`[ERROR] ${BGUTIL_TAG} has no server/ directory`);
    process.exit(1);
  }

  // canvas has no ARM64 prebuild and the server does not need it, so it
  // would fail windows-11-arm for nothing.
  const packageJsonPath = join(serverDir, "package.json");
  const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
  if (packageJson.dependencies?.canvas) {
    delete packageJson.dependencies.canvas;
    writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`, "utf8");
    console.log("[INFO] Dropped the canvas dependency");
  }

  // typescript is already one of the package's devDependencies, so a full
  // install is all the compile needs. `npm prune --omit=dev` afterwards is
  // not dependable — it removed 141 packages here and nothing at all on the
  // runner, which is how a 11 MB zip shipped as 26 MB. Throwing the tree
  // away and reinstalling production-only gives the same result everywhere.
  run("npm", ["install", "--omit=optional", "--no-audit", "--no-fund"], serverDir);
  run("npx", ["tsc"], serverDir);
  if (!existsSync(join(serverDir, "build", "main.js"))) {
    console.error("[ERROR] build/main.js missing after compile");
    process.exit(1);
  }
  rmSync(join(serverDir, "node_modules"), { recursive: true, force: true });
  run("npm", ["install", "--omit=dev", "--omit=optional", "--no-audit", "--no-fund"], serverDir);

  // The zip's root must hold exactly build/, node_modules/ and package.json,
  // which is what the app unpacks. Stage those three and let .NET zip the
  // staging directory: it needs no external tool, and Compress-Archive
  // crawls over five thousand files.
  const stageDir = join(workDir, "stage");
  mkdirSync(stageDir, { recursive: true });
  renameSync(join(serverDir, "build"), join(stageDir, "build"));
  renameSync(join(serverDir, "node_modules"), join(stageDir, "node_modules"));
  cpSync(packageJsonPath, join(stageDir, "package.json"));

  mkdirSync(binariesDir, { recursive: true });
  rmSync(target, { force: true });
  run(
    "powershell",
    [
      "-NoProfile",
      "-Command",
      `Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::CreateFromDirectory('${stageDir.replace(/'/g, "''")}', '${target.replace(/'/g, "''")}')`,
    ],
    workDir,
  );
  rmSync(workDir, { recursive: true, force: true });
  console.log(`[INFO] Built bgutil-pot-server.zip (${megabytes(target)} MB)`);
}

/** Resolves the handful of glob shapes the Windows config actually uses. */
function matches(pattern) {
  const full = resolve(tauriDir, pattern);
  if (!pattern.includes("*")) {
    return existsSync(full) ? [full] : [];
  }
  const dir = dirname(full);
  const suffix = full.slice(full.lastIndexOf("*") + 1);
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((name) => name.endsWith(suffix))
    .map((name) => join(dir, name));
}

function verifyDeclaredResources() {
  const configPath = join(tauriDir, "tauri.windows.conf.json");
  const config = JSON.parse(readFileSync(configPath, "utf8"));
  const declared = config.bundle?.resources ?? {};
  const patterns = Array.isArray(declared) ? declared : Object.keys(declared);
  const missing = patterns.filter((pattern) => matches(pattern).length === 0);
  for (const pattern of patterns) {
    const count = matches(pattern).length;
    console.log(`[${count ? "INFO" : "ERROR"}] ${pattern} -> ${count} file(s)`);
  }
  if (missing.length) {
    console.error(
      `[ERROR] ${missing.length} declared resource(s) missing; the bundle would fail on them one at a time.`,
    );
    process.exit(1);
  }
  console.log(`[INFO] All ${patterns.length} declared Windows resources are present.`);
}

await ensureYtDlp();
ensurePotServer();
verifyDeclaredResources();

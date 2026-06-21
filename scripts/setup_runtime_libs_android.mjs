#!/usr/bin/env node
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const filename = fileURLToPath(import.meta.url);
const scriptDir = dirname(filename);
const projectRoot = dirname(scriptDir);
const sourceRoot = resolve(projectRoot, "src-tauri", "libs", "mpv", "android");
const jniLibsRoot = resolve(projectRoot, "src-tauri", "gen", "android", "app", "src", "main", "jniLibs");
const supportedAbis = new Set(["arm64-v8a", "armeabi-v7a", "x86", "x86_64"]);

function printUsage() {
  console.log("Usage: node ./scripts/setup_runtime_libs_android.mjs");
}

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  printUsage();
  process.exit(0);
}

function copyDirectory(sourceDir, destinationDir) {
  mkdirSync(destinationDir, { recursive: true });

  for (const entry of readdirSync(sourceDir, { withFileTypes: true })) {
    const sourcePath = resolve(sourceDir, entry.name);
    const destinationPath = resolve(destinationDir, entry.name);
    if (entry.isDirectory()) {
      copyDirectory(sourcePath, destinationPath);
    } else if (entry.isFile()) {
      copyFileSync(sourcePath, destinationPath);
    }
  }
}

if (!existsSync(sourceRoot) || !statSync(sourceRoot).isDirectory()) {
  console.error(`[ERROR] Android runtime source directory not found: ${sourceRoot}`);
  process.exit(1);
}

const abiDirs = readdirSync(sourceRoot, { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && supportedAbis.has(entry.name))
  .map((entry) => entry.name)
  .sort((a, b) => a.localeCompare(b));

if (abiDirs.length === 0) {
  console.error(`[ERROR] No Android ABI runtime directories found under: ${sourceRoot}`);
  process.exit(1);
}

for (const abi of supportedAbis) {
  rmSync(resolve(jniLibsRoot, abi), { force: true, recursive: true });
}

for (const abi of abiDirs) {
  const sourceDir = resolve(sourceRoot, abi);
  const destinationDir = resolve(jniLibsRoot, abi);
  const libmpvPath = resolve(sourceDir, "libmpv.so");
  const soiaUtilsPath = resolve(sourceDir, "libsoia_utils.so");

  if (!existsSync(libmpvPath)) {
    console.error(`[ERROR] Missing Android libmpv runtime: ${libmpvPath}`);
    process.exit(1);
  }

  if (!existsSync(soiaUtilsPath)) {
    console.error(`[ERROR] Missing Android soia_utils runtime: ${soiaUtilsPath}`);
    process.exit(1);
  }

  copyDirectory(sourceDir, destinationDir);
  console.log(`[INFO] Synced Android runtime libs: ${abi}`);
}

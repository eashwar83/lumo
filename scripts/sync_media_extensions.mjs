#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const isCheckMode = process.argv.includes("--check");
const targetPlatform = process.platform;

const sourcePath = path.join(root, "src", "constants", "mediaExtensions.json");
const tauriRuntimeMacPath = path.join(root, "src-tauri", "tauri.runtime.macos.json");
const tauriWindowsConfigPath = path.join(root, "src-tauri", "tauri.windows.conf.json");
const associationName = "Lumo Media";
const defaultAssociation = {
  name: "Lumo Media",
  description: "Media file",
  role: "Viewer",
};

function readUtf8(filePath) {
  return fs.readFileSync(filePath, "utf8");
}

function writeUtf8(filePath, content) {
  fs.writeFileSync(filePath, content, "utf8");
}

function normalizeExtensions(value, options = {}) {
  const { allowEmpty = false, excludeImageKind = false } = options;
  if (!Array.isArray(value)) {
    throw new Error("mediaExtensions.json must be a JSON array");
  }

  const deduped = [];
  const seen = new Set();

  for (const item of value) {
    let rawExt = "";
    let kind = "";
    if (typeof item === "string") {
      rawExt = item;
    } else if (item && typeof item === "object" && typeof item.ext === "string") {
      rawExt = item.ext;
      kind = typeof item.kind === "string" ? item.kind.trim().toLowerCase() : "";
    } else {
      throw new Error(
        "mediaExtensions.json entries must be strings or objects with an ext field",
      );
    }

    if (excludeImageKind && kind === "image") {
      continue;
    }

    const normalized = rawExt.trim().toLowerCase();
    if (!normalized || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    deduped.push(normalized);
  }

  if (!allowEmpty && deduped.length === 0) {
    throw new Error("mediaExtensions.json is empty after normalization");
  }

  return deduped;
}

function arraysEqual(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function selectTargetConfigPath() {
  if (targetPlatform === "darwin") {
    return tauriRuntimeMacPath;
  }
  return tauriWindowsConfigPath;
}

function ensureRuntimeBase(config) {
  if (!config.bundle || typeof config.bundle !== "object") {
    config.bundle = {};
  }
}

function main() {
  const sourceExtensions = normalizeExtensions(JSON.parse(readUtf8(sourcePath)), {
    excludeImageKind: true,
  });
  const targetPath = selectTargetConfigPath();
  const targetLabel = path.relative(root, targetPath).replaceAll("\\", "/");
  const runtimeConfig = JSON.parse(readUtf8(targetPath));

  ensureRuntimeBase(runtimeConfig);

  const associations = Array.isArray(runtimeConfig.bundle.fileAssociations)
    ? runtimeConfig.bundle.fileAssociations
    : [];
  runtimeConfig.bundle.fileAssociations = associations;

  const association =
    associations.find((item) => item?.name === associationName) ??
    (() => {
      const created = { ...defaultAssociation, ext: [] };
      associations.push(created);
      return created;
    })();

  if (typeof association !== "object" || !association) {
    throw new Error("No valid file association item found in runtime config.");
  }

  const currentExtensions = normalizeExtensions(
    Array.isArray(association.ext) ? association.ext : [],
    { allowEmpty: true },
  );
  if (arraysEqual(currentExtensions, sourceExtensions)) {
    console.log("Media extensions already in sync.");
    return;
  }

  if (isCheckMode) {
    console.error("Media extensions are out of sync:");
    console.error(`- source: src/constants/mediaExtensions.json (${sourceExtensions.length})`);
    console.error(`- tauri : ${targetLabel} (${currentExtensions.length})`);
    process.exit(1);
  }

  association.ext = sourceExtensions;
  writeUtf8(targetPath, `${JSON.stringify(runtimeConfig, null, 2)}\n`);
  console.log(`Synced ${sourceExtensions.length} media extensions to ${targetLabel}.`);
}

main();

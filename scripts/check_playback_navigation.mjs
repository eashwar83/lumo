import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import ts from "typescript";

const tempDir = await mkdtemp(path.join(tmpdir(), "soia-playback-navigation-"));
const tempModulePath = path.join(tempDir, "playbackNavigation.mjs");

try {
  const sourcePath = new URL("../src/utils/playbackNavigation.ts", import.meta.url);
  const source = await readFile(sourcePath, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ES2020,
      module: ts.ModuleKind.ES2020,
    },
  });
  await writeFile(tempModulePath, compiled.outputText);

  const { resolvePlaybackNavigationPath } = await import(
    pathToFileURL(tempModulePath).href
  );

  {
    let directoryCalls = 0;
    const result = await resolvePlaybackNavigationPath({
      currentPath: "/media/current.mkv",
      direction: 1,
      resolvePlaylistPath: () => "/playlist/next.mkv",
      resolveDirectoryPath: async () => {
        directoryCalls += 1;
        return "/directory/next.mkv";
      },
    });
    assert.equal(result, "/playlist/next.mkv");
    assert.equal(directoryCalls, 0);
  }

  {
    const result = await resolvePlaybackNavigationPath({
      currentPath: "/media/current.mkv",
      direction: 1,
      resolvePlaylistPath: () => null,
      resolveDirectoryPath: async () => "/directory/next.mkv",
    });
    assert.equal(result, "/directory/next.mkv");
  }

  {
    const seenDirections = [];
    const result = await resolvePlaybackNavigationPath({
      currentPath: "/media/current.mkv",
      direction: -1,
      resolvePlaylistPath: (_currentPath, direction) => {
        seenDirections.push(direction);
        return null;
      },
      resolveDirectoryPath: async (_currentPath, direction) => {
        seenDirections.push(direction);
        return "/directory/previous.mkv";
      },
    });
    assert.equal(result, "/directory/previous.mkv");
    assert.deepEqual(seenDirections, [-1, -1]);
  }

  {
    let playlistCalls = 0;
    let directoryCalls = 0;
    const result = await resolvePlaybackNavigationPath({
      currentPath: "   ",
      direction: 1,
      resolvePlaylistPath: () => {
        playlistCalls += 1;
        return "/playlist/next.mkv";
      },
      resolveDirectoryPath: async () => {
        directoryCalls += 1;
        return "/directory/next.mkv";
      },
    });
    assert.equal(result, null);
    assert.equal(playlistCalls, 0);
    assert.equal(directoryCalls, 0);
  }
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

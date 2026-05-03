import { invoke } from "@tauri-apps/api/core";
import { MEDIA_FILE_EXTENSIONS } from "../constants/media";
import type { NetworkBrowseEntry, NetworkBrowseResult } from "../types/network";
import { createWebdavPlaybackKey, parsePlaybackSource } from "../utils/playbackSource";

const mediaExtensionSet = new Set(
    MEDIA_FILE_EXTENSIONS.map((item) => item.toLowerCase()),
);

const getLowerExt = (path: string): string => {
    const index = path.lastIndexOf(".");
    if (index < 0) return "";
    return path.slice(index + 1).toLowerCase();
};

const isMediaFilePath = (path: string): boolean =>
    mediaExtensionSet.has(getLowerExt(path));

const normalizeWebdavPath = (path: string): string => {
    const trimmed = path.trim();
    if (!trimmed || trimmed === "/") return "/";
    const withLeading = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
    return withLeading.replace(/\/+$/, "");
};

const getWebdavParentPath = (path: string): string | null => {
    const normalized = normalizeWebdavPath(path);
    if (normalized === "/") return null;
    const index = normalized.lastIndexOf("/");
    if (index <= 0) return "/";
    return normalized.slice(0, index);
};

const resolveAdjacentPathInLocalDirectory = async (
    currentPath: string,
    direction: 1 | -1,
): Promise<string | null> => {
    if (!isMediaFilePath(currentPath)) return null;
    const siblings = await invoke<string[]>("list_local_media_siblings", {
        path: currentPath,
    }).catch(() => []);
    if (!siblings.length) return null;
    const mediaSiblings = siblings.filter((item) => isMediaFilePath(item));
    const currentIndex = mediaSiblings.findIndex((item) => item === currentPath);
    const nextIndex = currentIndex + direction;
    if (
        currentIndex < 0 ||
        nextIndex < 0 ||
        nextIndex >= mediaSiblings.length
    ) {
        return null;
    }
    return mediaSiblings[nextIndex] ?? null;
};

const resolveAdjacentPathInWebdavDirectory = async (
    connectionId: string,
    filePath: string,
    direction: 1 | -1,
): Promise<string | null> => {
    const parentPath = getWebdavParentPath(filePath);
    if (!parentPath) return null;
    const result = await invoke<NetworkBrowseResult>("browse_network_connection", {
        payload: {
            connectionId,
            mode: "browse",
            protocol: "webdav",
            path: parentPath,
        },
    }).catch(() => null);
    if (!result) return null;
    const mediaFiles = result.entries.filter(
        (entry: NetworkBrowseEntry) =>
            entry.entryType === "file" && isMediaFilePath(entry.path),
    );
    const normalizedCurrentPath = normalizeWebdavPath(filePath);
    const currentIndex = mediaFiles.findIndex(
        (entry) => normalizeWebdavPath(entry.path) === normalizedCurrentPath,
    );
    const nextIndex = currentIndex + direction;
    if (
        currentIndex < 0 ||
        nextIndex < 0 ||
        nextIndex >= mediaFiles.length
    ) {
        return null;
    }
    const nextEntry = mediaFiles[nextIndex];
    return createWebdavPlaybackKey(connectionId, nextEntry.path);
};

export const resolveAdjacentPathInSameDirectory = async (
    currentPath: string,
    direction: 1 | -1,
): Promise<string | null> => {
    const source = parsePlaybackSource(currentPath);
    if (source.type === "local") {
        return resolveAdjacentPathInLocalDirectory(source.path, direction);
    }
    if (source.type === "dlna") {
        return null;
    }
    return resolveAdjacentPathInWebdavDirectory(
        source.connectionId,
        source.filePath,
        direction,
    );
};

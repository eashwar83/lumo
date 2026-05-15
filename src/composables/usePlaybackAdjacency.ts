import type { NetworkBrowseEntry, NetworkBrowseResult } from "../types/network";
import { invoke } from "@tauri-apps/api/core";
import {
    isMediaFilePath,
    listLocalSiblingFiles,
    normalizeLocalPathForCompare,
} from "../utils/localMediaSiblings";
import {
    createDlnaPlaybackKey,
    createWebdavPlaybackKey,
    parsePlaybackSource,
} from "../utils/playbackSource";

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

const normalizeDlnaObjectId = (path: string): string => path.trim() || "0";

const resolveAdjacentPathInLocalDirectory = async (
    currentPath: string,
    direction: 1 | -1,
): Promise<string | null> => {
    if (!isMediaFilePath(currentPath)) return null;
    const siblings = await listLocalSiblingFiles(currentPath);
    if (!siblings.length) return null;
    const mediaSiblings = siblings.filter((item) => isMediaFilePath(item));
    const normalizedCurrentPath = normalizeLocalPathForCompare(currentPath);
    const currentIndex = mediaSiblings.findIndex(
        (item) => normalizeLocalPathForCompare(item) === normalizedCurrentPath,
    );
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

const resolveAdjacentPathInDlnaDirectory = async (
    connectionId: string,
    resourceUrl: string,
    parentPath: string | undefined,
    direction: 1 | -1,
): Promise<string | null> => {
    if (!parentPath) return null;
    const normalizedParentPath = normalizeDlnaObjectId(parentPath);
    const result = await invoke<NetworkBrowseResult>("browse_network_connection", {
        payload: {
            connectionId,
            mode: "browse",
            protocol: "http-dlna",
            path: normalizedParentPath,
        },
    }).catch(() => null);
    if (!result) return null;
    const mediaFiles = result.entries.filter(
        (entry: NetworkBrowseEntry) => entry.entryType === "file",
    );
    const currentIndex = mediaFiles.findIndex(
        (entry) => entry.path === resourceUrl,
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
    return createDlnaPlaybackKey(
        connectionId,
        nextEntry.path,
        normalizedParentPath,
    );
};

const resolveAdjacentPathInSmbDirectory = async (
    _currentUrl: string,
    _direction: 1 | -1,
): Promise<string | null> => {
    return null;
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
        return resolveAdjacentPathInDlnaDirectory(
            source.connectionId,
            source.resourceUrl,
            source.parentPath,
            direction,
        );
    }
    if (source.type === "smb") {
        return resolveAdjacentPathInSmbDirectory(source.url ?? source.key, direction);
    }
    return resolveAdjacentPathInWebdavDirectory(
        source.connectionId,
        source.filePath,
        direction,
    );
};

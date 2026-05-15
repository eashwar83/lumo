import { invoke } from "@tauri-apps/api/core";
import { MEDIA_FILE_EXTENSIONS } from "../constants/media";

const mediaExtensionSet = new Set(
    MEDIA_FILE_EXTENSIONS.map((item) => item.toLowerCase()),
);

const getFileName = (path: string): string => {
    const normalized = path.split(/[?#]/, 1)[0] ?? path;
    const slashIndex = Math.max(
        normalized.lastIndexOf("/"),
        normalized.lastIndexOf("\\"),
    );
    return slashIndex >= 0 ? normalized.slice(slashIndex + 1) : normalized;
};

export const normalizeLocalPathForCompare = (path: string): string => {
    const trimmed = path.trim();
    if (!/^file:\/\//i.test(trimmed)) return trimmed;
    try {
        const url = new URL(trimmed);
        const decodedPath = decodeURIComponent(url.pathname);
        return decodedPath.replace(/^\/(?=[A-Za-z]:)/, "");
    } catch {
        return trimmed;
    }
};

const getLowerExt = (path: string): string => {
    const fileName = getFileName(path);
    const index = fileName.lastIndexOf(".");
    if (index < 0) return "";
    return fileName.slice(index + 1).toLowerCase();
};

export const isMediaFilePath = (path: string): boolean =>
    mediaExtensionSet.has(getLowerExt(path));

export const listLocalSiblingFiles = async (
    currentPath: string,
): Promise<string[]> =>
    await invoke<string[]>("list_local_media_siblings", {
        path: currentPath,
    }).catch(() => []);

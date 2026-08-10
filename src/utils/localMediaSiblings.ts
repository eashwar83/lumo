import { invoke } from "@tauri-apps/api/core";

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

export const listLocalSiblingFiles = async (
    currentPath: string,
): Promise<string[]> =>
    await invoke<string[]>("list_local_media_siblings", {
        path: currentPath,
    }).catch(() => []);

import { computed, nextTick, type Ref, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
    NetworkBrowseEntry,
    NetworkBrowseResult,
    NetworkFileRow,
    NetworkConnection,
} from "../types/network";

const normalizePath = (path: string) => {
    const trimmed = path.trim();
    if (!trimmed || trimmed === "/") return "/";
    const withLeading = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
    return withLeading.replace(/\/+$/, "");
};

const getParentPath = (path: string) => {
    if (!path.startsWith("/")) return null;
    const normalized = normalizePath(path);
    if (normalized === "/") return null;
    const index = normalized.lastIndexOf("/");
    if (index < 0) return null;
    if (index === 0) return "/";
    return normalized.slice(0, index);
};

const normalizeObjectId = (value: string) => {
    const trimmed = value.trim();
    return trimmed || "0";
};

const formatSize = (bytes: number | null) => {
    if (bytes == null || Number.isNaN(bytes)) return "—";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit += 1;
    }
    return unit === 0 ? `${Math.round(value)} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
};

const formatModified = (value: string | null) => value ?? "";

const mapBrowseEntry = (entry: NetworkBrowseEntry): NetworkFileRow => ({
    name: entry.name,
    path: entry.path,
    type: entry.entryType === "dir" ? "DIR" : "FILE",
    size: formatSize(entry.size),
    modified: formatModified(entry.modifiedAt),
});

export const useNetworkBrowser = (
    selectedConnectionId: Ref<string>,
    selectedConnection: Ref<NetworkConnection | null>,
) => {
    const networkPath = ref("/");
    const currentFiles = ref<NetworkFileRow[]>([]);
    const isLoading = ref(false);
    const errorMessage = ref("");
    const browseCache = new Map<string, Map<string, NetworkFileRow[]>>();

    const getConnectionCache = (connectionId: string) => {
        let cache = browseCache.get(connectionId);
        if (!cache) {
            cache = new Map();
            browseCache.set(connectionId, cache);
        }
        return cache;
    };

    const readCache = (connectionId: string, path: string) => {
        const cache = browseCache.get(connectionId);
        if (!cache) return null;
        return cache.get(path) ?? null;
    };

    const writeCache = (connectionId: string, path: string, entries: NetworkFileRow[]) => {
        const cache = getConnectionCache(connectionId);
        cache.set(path, entries);
    };

    const browse = async (
        path: string,
        mode: "connect" | "browse",
        options: { allowCache?: boolean } = {},
    ) => {
        const connectionId = selectedConnectionId.value;
        if (!connectionId) return;
        const protocol = selectedConnection.value?.protocol?.toLowerCase() ?? "webdav";
        const isDlna = protocol === "http-dlna" || protocol === "dlna";
        const targetPath = isDlna ? normalizeObjectId(path) : normalizePath(path);
        const allowCache = Boolean(options.allowCache);
        if (allowCache) {
            const cached = readCache(connectionId, targetPath);
            if (cached) {
                networkPath.value = targetPath;
                currentFiles.value = cached.map((entry) => ({ ...entry }));
                errorMessage.value = "";
                return;
            }
        }
        const previousPath = networkPath.value;
        const previousFiles = [...currentFiles.value];
        const isDirectoryBrowse = mode === "browse";
        const isConnect = mode === "connect";
        const shouldOptimisticNavigate = isDirectoryBrowse || isConnect;

        if (shouldOptimisticNavigate) {
            // Optimistically enter target directory for immediate UI feedback.
            networkPath.value = targetPath;
            currentFiles.value = [];
        }

        isLoading.value = true;
        errorMessage.value = "";
        await nextTick();
        try {
            const result = await invoke<NetworkBrowseResult>("browse_network_connection", {
                payload: {
                    connectionId,
                    mode,
                    protocol,
                    path: targetPath,
                },
            });
            networkPath.value = isDlna
                ? normalizeObjectId(result.path)
                : normalizePath(result.path);
            currentFiles.value = result.entries.map(mapBrowseEntry);
            writeCache(connectionId, networkPath.value, currentFiles.value);
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : typeof error === "string"
                      ? error
                      : "Network browse failed";
            errorMessage.value = message;
            if (isDirectoryBrowse) {
                networkPath.value = previousPath;
                currentFiles.value = previousFiles;
            } else {
                currentFiles.value = [];
            }
            throw error;
        } finally {
            isLoading.value = false;
        }
    };

    const connect = async () => browse(networkPath.value, "connect");

    const refresh = async () => browse(networkPath.value, "browse");

    const openDirectory = async (path: string) => {
        await browse(path, "browse", { allowCache: true });
    };

    const parentPath = computed(() => getParentPath(networkPath.value));

    const networkEntries = computed(() => {
        const entries = [...currentFiles.value];
        if (parentPath.value) {
            entries.unshift({
                name: "..",
                path: parentPath.value,
                type: "DIR",
                size: "—",
                modified: "",
                isParent: true,
            });
        }
        return entries;
    });

    const hasFiles = computed(() => currentFiles.value.length > 0);

    return {
        networkPath,
        currentFiles,
        networkEntries,
        isLoading,
        errorMessage,
        hasFiles,
        connect,
        refresh,
        openDirectory,
        normalizePath,
        getParentPath,
    };
};

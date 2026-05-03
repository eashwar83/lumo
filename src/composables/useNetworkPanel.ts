import { computed, onMounted, ref, watch } from "vue";
import type { NetworkFileRow, NetworkPlayRequest } from "../types/network";
import {
    createDlnaPlaybackKey,
    createWebdavPlaybackKey,
} from "../utils/playbackSource";
import { useNetworkConnections } from "./useNetworkConnections";
import { useNetworkBrowser } from "./useNetworkBrowser";
import {
    createDebouncedUiStateSaver,
    loadUiState as loadUiStateStore,
} from "./useUiStateStore";

type StoredNetworkState = {
    selectedConnection?: string;
    path?: string;
};

type NetworkPanelViewMode = "connections" | "browser";

export const useNetworkPanel = () => {
    const {
        connections,
        selectedConnectionId,
        selectedConnection,
        loadConnections,
        createConnection,
        saveSelectedConnection,
        deleteSelectedConnection,
        discoverConnections,
    } = useNetworkConnections();
    const browser = useNetworkBrowser(selectedConnectionId, selectedConnection);
    const viewMode = ref<NetworkPanelViewMode>("connections");
    const hasConnected = ref(false);
    const activeConnectionLabel = computed(
        () => selectedConnection.value?.label || "Network",
    );
    const connectionStatus = computed(() =>
        browser.errorMessage.value
            ? "Error"
            : hasConnected.value
              ? "Connected"
              : "Idle",
    );

    const uiStateSaver = createDebouncedUiStateSaver(300);

    const toErrorMessage = (error: unknown, fallback: string) => {
        if (error instanceof Error && error.message) return error.message;
        if (typeof error === "string" && error) return error;
        return fallback;
    };

    const saveUiState = () => {
        const protocol = selectedConnection.value?.protocol?.toLowerCase() ?? "webdav";
        const isDlna = protocol === "http-dlna" || protocol === "dlna";
        uiStateSaver.saveDebounced({
            network: {
                selectedConnection: selectedConnectionId.value,
                path: isDlna ? "0" : browser.networkPath.value,
            },
        });
    };

    const loadStoredUiState = async () => {
        const stored = await loadUiStateStore<{ network?: StoredNetworkState }>();
        const network = stored?.network;
        if (network?.selectedConnection) {
            selectedConnectionId.value = network.selectedConnection;
        }
        if (network?.path) {
            browser.networkPath.value =
                selectedConnection.value?.protocol === "http-dlna"
                    ? "0"
                    : browser.normalizePath(network.path);
        }
    };

    const onConnect = async () => {
        try {
            await saveSelectedConnection();
            await browser.connect();
            hasConnected.value = true;
        } catch (error) {
            hasConnected.value = false;
            browser.errorMessage.value = toErrorMessage(
                error,
                "Failed to connect network server",
            );
        }
    };

    const onOpenBrowser = async (connectionId?: string) => {
        if (connectionId) {
            selectedConnectionId.value = connectionId;
        }
        const connection = selectedConnection.value;
        if (!connection) return;
        browser.networkPath.value =
            connection.protocol === "http-dlna"
                ? "0"
                : browser.normalizePath(connection.defaultPath || "/");
        viewMode.value = "browser";
        await onConnect();
    };

    const onBackToConnections = () => {
        viewMode.value = "connections";
    };

    const onRefresh = async () => {
        try {
            await browser.refresh();
            hasConnected.value = true;
        } catch (error) {
            browser.errorMessage.value = toErrorMessage(
                error,
                "Failed to refresh directory",
            );
        }
    };

    const onSaveConnection = async () => {
        try {
            await saveSelectedConnection();
            browser.errorMessage.value = "";
        } catch (error) {
            browser.errorMessage.value = toErrorMessage(
                error,
                "Failed to save connection",
            );
        }
    };

    const onCreateConnection = () => {
        createConnection();
        viewMode.value = "connections";
    };

    const onDeleteConnection = async () => {
        try {
            await deleteSelectedConnection();
            browser.currentFiles.value = [];
            browser.errorMessage.value = "";
            hasConnected.value = false;
            viewMode.value = "connections";
        } catch (error) {
            browser.errorMessage.value = toErrorMessage(
                error,
                "Failed to delete connection",
            );
        }
    };

    const onDiscoverConnections = async () => {
        await discoverConnections();
    };

    const openEntry = async (entry: NetworkFileRow) => {
        if (entry.type !== "DIR") return;
        await browser.openDirectory(entry.path);
    };

    const onBrowsePath = async (path: string) => {
        await browser.openDirectory(path);
    };

    const buildPlayRequest = (entry: NetworkFileRow): NetworkPlayRequest => ({
        protocol: selectedConnection.value?.protocol || "webdav",
        connectionId: selectedConnectionId.value,
        filePath: entry.path,
        playbackKey:
            selectedConnection.value?.protocol === "http-dlna" ||
            selectedConnection.value?.protocol === "dlna"
                ? createDlnaPlaybackKey(
                    selectedConnectionId.value,
                    entry.path,
                    browser.networkPath.value,
                )
                : createWebdavPlaybackKey(selectedConnectionId.value, entry.path),
        displayName: entry.name,
    });

    watch([selectedConnectionId, browser.networkPath], saveUiState);

    watch(
        selectedConnection,
        (connection) => {
            if (!connection) return;
            if (!connection.defaultPath) {
                connection.defaultPath =
                    connection.protocol === "http-dlna" ? "0" : "/";
            }
        },
        { immediate: true },
    );

    watch(
        () => selectedConnectionId.value,
        () => {
            hasConnected.value = false;
        },
    );

    watch(
        () => browser.networkPath.value,
        (path) => {
            if (!selectedConnection.value) return;
            if (selectedConnection.value.protocol === "http-dlna") return;
            selectedConnection.value.defaultPath = path;
        },
    );

    onMounted(async () => {
        await loadConnections();
        await loadStoredUiState();
        try {
            await onDiscoverConnections();
        } catch {
            // Keep startup non-blocking even when discovery fails.
        }
        if (!selectedConnection.value && connections.value.length > 0) {
            selectedConnectionId.value = connections.value[0].id;
        }
    });

    return {
        viewMode,
        activeConnectionLabel,
        networkConnections: connections,
        selectedConnection: selectedConnectionId,
        selectedConnectionConfig: selectedConnection,
        networkEntries: browser.networkEntries,
        pathCrumbs: browser.pathCrumbs,
        parentPath: browser.parentPath,
        networkPath: browser.networkPath,
        isLoading: browser.isLoading,
        connectionStatus,
        errorMessage: browser.errorMessage,
        hasFiles: browser.hasFiles,
        openEntry,
        onBrowsePath,
        onConnect,
        onRefresh,
        onSaveConnection,
        onCreateConnection,
        onDeleteConnection,
        onDiscoverConnections,
        onOpenBrowser,
        onBackToConnections,
        buildPlayRequest,
        getAncestorPaths: browser.getAncestorPaths,
    };
};

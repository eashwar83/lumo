import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { NetworkConnection } from "../types/network";

const shouldUseMockConnections = import.meta.env.DEV;

const createConnectionId = () => `webdav-${Date.now()}`;
const createDlnaConnectionId = (usn: string, index: number) =>
    `dlna-${usn.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || index}`;

type DiscoveredNetworkConnection = {
    protocol: string;
    usn: string | null;
    location: string;
    friendlyName: string | null;
    server: string | null;
    st: string | null;
};

const hostFromLocation = (location: string) => {
    try {
        const url = new URL(location);
        return url.hostname || null;
    } catch {
        return null;
    }
};

const toConnectionFromDiscovery = (
    connection: DiscoveredNetworkConnection,
    index: number,
): NetworkConnection | null => {
    const protocol = connection.protocol.trim().toLowerCase();
    if (protocol === "http-dlna" || protocol === "dlna") {
        const host = hostFromLocation(connection.location);
        return {
            id: createDlnaConnectionId(connection.usn || connection.location, index),
            label:
                connection.friendlyName?.trim() ||
                host ||
                connection.server?.split("/")[0]?.trim() ||
                `DLNA ${index + 1}`,
            protocol: "http-dlna",
            baseUrl: connection.location,
            username: "",
            password: "",
            defaultPath: "0",
        };
    }
    return null;
};

const cloneConnection = (connection: NetworkConnection): NetworkConnection => ({
    ...connection,
});

const mockSeedConnections: NetworkConnection[] = [
    {
        id: "mock-webdav-studio",
        label: "Studio NAS",
        protocol: "webdav",
        baseUrl: "http://192.168.31.18:1900/dav",
        username: "media",
        password: "",
        defaultPath: "/Movies",
    },
    {
        id: "mock-webdav-living-room",
        label: "Living Room Box",
        protocol: "webdav",
        baseUrl: "http://192.168.31.25:8080/webdav",
        username: "guest",
        password: "",
        defaultPath: "/Video",
    },
    {
        id: "mock-smb-family-share",
        label: "Family Share",
        protocol: "smb",
        baseUrl: "smb://192.168.31.33/media",
        username: "feng",
        password: "",
        defaultPath: "/tv",
    },
];

const mockDiscoveredConnections: NetworkConnection[] = [
    {
        id: "mock-webdav-office-server",
        label: "Office Server",
        protocol: "webdav",
        baseUrl: "http://192.168.31.29:8080/dav",
        username: "admin",
        password: "",
        defaultPath: "/shared",
    },
    {
        id: "mock-webdav-bedroom-pc",
        label: "Bedroom PC",
        protocol: "webdav",
        baseUrl: "http://192.168.31.42:9012/dav",
        username: "soia",
        password: "",
        defaultPath: "/downloads",
    },
    {
        id: "mock-smb-family-share",
        label: "Family Share",
        protocol: "smb",
        baseUrl: "smb://192.168.31.33/media",
        username: "feng",
        password: "",
        defaultPath: "/tv",
    },
    {
        id: "mock-ftp-backup-box",
        label: "Backup FTP",
        protocol: "ftp",
        baseUrl: "ftp://192.168.31.50:21/archive",
        username: "backup",
        password: "",
        defaultPath: "/incoming",
    },
    {
        id: "mock-http-dlna-mediahub",
        label: "MediaHub DLNA",
        protocol: "http-dlna",
        baseUrl: "http://192.168.31.66:8200/MediaServer",
        username: "",
        password: "",
        defaultPath: "/ContentDirectory",
    },
];

const createDraftConnection = (): NetworkConnection => ({
    id: createConnectionId(),
    label: "WebDAV",
    protocol: "webdav",
    baseUrl: "",
    username: "",
    password: "",
    defaultPath: "/",
});

export const useNetworkConnections = () => {
    const connections = ref<NetworkConnection[]>([]);
    const selectedConnectionId = ref("");

    const selectedConnection = computed(() => {
        if (!selectedConnectionId.value) return null;
        return (
            connections.value.find(
                (connection) => connection.id === selectedConnectionId.value,
            ) ?? null
        );
    });

    const ensureSelection = () => {
        if (
            selectedConnectionId.value &&
            connections.value.some(
                (connection) => connection.id === selectedConnectionId.value,
            )
        ) {
            return;
        }
        selectedConnectionId.value = connections.value[0]?.id ?? "";
    };

    const loadConnections = async () => {
        const loaded = await invoke<NetworkConnection[]>(
            "list_network_connections",
        );
        connections.value =
            loaded.length > 0
                ? loaded
                : shouldUseMockConnections
                  ? mockSeedConnections.map(cloneConnection)
                  : [];
        ensureSelection();
    };

    const createConnection = () => {
        const connection = createDraftConnection();
        connections.value.push(connection);
        selectedConnectionId.value = connection.id;
    };

    const saveSelectedConnection = async () => {
        const connection = selectedConnection.value;
        if (!connection) return;
        const saved = await invoke<NetworkConnection[]>("save_network_connection", {
            connection,
        });
        connections.value = saved;
        ensureSelection();
    };

    const deleteSelectedConnection = async () => {
        const connection = selectedConnection.value;
        if (!connection) return;
        const saved = await invoke<NetworkConnection[]>("delete_network_connection", {
            connectionId: connection.id,
        });
        connections.value = saved;
        ensureSelection();
    };

    const discoverConnections = async () => {
        try {
            const discoveredConnections = await invoke<DiscoveredNetworkConnection[]>(
                "discover_network_connections",
                {
                    payload: {
                        protocol: "all",
                    },
                },
            );
            if (!discoveredConnections.length) return 0;
            let updatedCount = 0;
            for (const discoveredConnection of discoveredConnections) {
                const mapped = toConnectionFromDiscovery(discoveredConnection, updatedCount);
                if (!mapped) continue;
                const existing = connections.value.find(
                    (connection) =>
                        connection.baseUrl.trim() === mapped.baseUrl.trim(),
                );
                if (!existing) continue;
                if (
                    existing.label !== mapped.label ||
                    existing.protocol !== mapped.protocol
                ) {
                    existing.label = mapped.label;
                    existing.protocol = mapped.protocol;
                    updatedCount += 1;
                }
            }

            const seenBaseUrls = new Set(
                connections.value.map((connection) => connection.baseUrl.trim()),
            );
            const discovered = discoveredConnections
                .filter((connection) => {
                    const location = connection.location.trim();
                    if (!location || seenBaseUrls.has(location)) return false;
                    seenBaseUrls.add(location);
                    return true;
                })
                .map((connection, index) =>
                    toConnectionFromDiscovery(connection, index),
                )
                .filter((item): item is NetworkConnection => Boolean(item));
            if (!discovered.length) return 0;
            connections.value = [...connections.value, ...discovered];
            ensureSelection();
            return discovered.length + updatedCount;
        } catch (error) {
            if (!shouldUseMockConnections) {
                throw error;
            }
        }

        const existingIds = new Set(
            connections.value.map((connection) => connection.id),
        );
        const discovered = mockDiscoveredConnections
            .filter((connection) => !existingIds.has(connection.id))
            .map(cloneConnection);
        if (!discovered.length) return 0;
        connections.value = [...connections.value, ...discovered];
        ensureSelection();
        return discovered.length;
    };

    return {
        connections,
        selectedConnectionId,
        selectedConnection,
        loadConnections,
        createConnection,
        saveSelectedConnection,
        deleteSelectedConnection,
        discoverConnections,
    };
};

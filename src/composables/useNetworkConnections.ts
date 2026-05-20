import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { NetworkConnection } from "../types/network";

const shouldUseMockConnections = import.meta.env.DEV;

const protocolIdPrefix = (protocol: string) => {
    const normalized = protocol.trim().toLowerCase();
    if (normalized === "smb" || normalized === "samba") return "smb";
    if (normalized === "http-dlna" || normalized === "dlna") return "dlna";
    if (normalized === "ftp") return "ftp";
    return "webdav";
};

const createConnectionId = (protocol = "webdav") =>
    `${protocolIdPrefix(protocol)}-${Date.now()}`;
const createDlnaConnectionId = (usn: string, index: number) =>
    `dlna-${usn.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || index}`;
const createSmbConnectionId = (usn: string, index: number) =>
    `smb-${usn.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || index}`;

const normalizeGeneratedConnectionIdForProtocol = (
    id: string,
    protocol: string,
) => {
    const expectedPrefix = protocolIdPrefix(protocol);
    const generatedMatch = id.trim().match(/^(webdav|smb|dlna|ftp)-(\d+)$/i);
    if (!generatedMatch) return id;
    const currentPrefix = generatedMatch[1]?.toLowerCase();
    if (currentPrefix === expectedPrefix) return id;
    return `${expectedPrefix}-${generatedMatch[2]}`;
};

type DiscoveredNetworkConnection = {
    protocol: string;
    usn: string | null;
    location: string;
    friendlyName: string | null;
    server: string | null;
    st: string | null;
};

type NetworkDiscoveryFoundPayload = {
    scanId: string | null;
    connection: DiscoveredNetworkConnection;
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
    if (protocol === "smb" || protocol === "samba") {
        const host = hostFromLocation(connection.location);
        return {
            id: createSmbConnectionId(connection.usn || connection.location, index),
            label:
                connection.friendlyName?.trim() ||
                host ||
                connection.server?.split(".")[0]?.trim() ||
                `SMB ${index + 1}`,
            protocol: "smb",
            baseUrl: connection.location,
            username: "",
            password: "",
            defaultPath: "/",
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

const createDraftConnection = (protocol = "webdav"): NetworkConnection => ({
    id: createConnectionId(protocol),
    label: "WebDAV",
    protocol,
    baseUrl: "",
    username: "",
    password: "",
    defaultPath: "/",
});

export const useNetworkConnections = () => {
    const connections = ref<NetworkConnection[]>([]);
    const selectedConnectionId = ref("");
    const activeDiscoveryScanId = ref<string | null>(null);
    let discoveryEventUnlisten: UnlistenFn | null = null;
    let discoveryEventListenPromise: Promise<void> | null = null;

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

    const mergeDiscoveredConnection = (
        discoveredConnection: DiscoveredNetworkConnection,
        index: number,
    ) => {
        const mapped = toConnectionFromDiscovery(discoveredConnection, index);
        if (!mapped) return false;
        const mappedBaseUrl = mapped.baseUrl.trim();
        if (!mappedBaseUrl) return false;

        const existing = connections.value.find((connection) => {
            const existingBaseUrl = connection.baseUrl.trim();
            return connection.id === mapped.id || existingBaseUrl === mappedBaseUrl;
        });
        if (existing) {
            let changed = false;
            if (existing.baseUrl !== mapped.baseUrl) {
                existing.baseUrl = mapped.baseUrl;
                changed = true;
            }
            if (existing.label !== mapped.label) {
                existing.label = mapped.label;
                changed = true;
            }
            if (existing.protocol !== mapped.protocol) {
                existing.protocol = mapped.protocol;
                changed = true;
            }
            return changed;
        }

        connections.value = [...connections.value, mapped];
        ensureSelection();
        return true;
    };

    const ensureDiscoveryEventListener = async () => {
        if (discoveryEventUnlisten) return;
        if (discoveryEventListenPromise) {
            await discoveryEventListenPromise;
            return;
        }

        discoveryEventListenPromise = listen<NetworkDiscoveryFoundPayload>(
            "network-discovery-found",
            (event) => {
                if (
                    event.payload.scanId &&
                    event.payload.scanId !== activeDiscoveryScanId.value
                ) {
                    return;
                }
                mergeDiscoveredConnection(
                    event.payload.connection,
                    connections.value.length,
                );
            },
        ).then((unlisten) => {
            discoveryEventUnlisten = unlisten;
        });

        await discoveryEventListenPromise;
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

    const createConnection = (protocol = "webdav") => {
        const connection = createDraftConnection(protocol);
        connections.value.push(connection);
        selectedConnectionId.value = connection.id;
    };

    const saveSelectedConnection = async () => {
        const connection = selectedConnection.value;
        if (!connection) return;
        const oldId = connection.id;
        connection.id = normalizeGeneratedConnectionIdForProtocol(
            connection.id,
            connection.protocol,
        );
        let saved = await invoke<NetworkConnection[]>("save_network_connection", {
            connection,
        });
        if (oldId !== connection.id) {
            saved = await invoke<NetworkConnection[]>("delete_network_connection", {
                connectionId: oldId,
            });
            selectedConnectionId.value = connection.id;
        }
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
            await ensureDiscoveryEventListener();
            const scanId = `${Date.now()}-${Math.random()
                .toString(36)
                .slice(2)}`;
            activeDiscoveryScanId.value = scanId;
            const discoveredConnections = await invoke<DiscoveredNetworkConnection[]>(
                "discover_network_connections",
                {
                    payload: {
                        protocol: "all",
                        scanId,
                    },
                },
            );
            if (!discoveredConnections.length) return 0;
            let updatedCount = 0;
            for (const discoveredConnection of discoveredConnections) {
                if (
                    mergeDiscoveredConnection(
                        discoveredConnection,
                        connections.value.length + updatedCount,
                    )
                ) {
                    updatedCount += 1;
                }
            }
            return updatedCount;
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

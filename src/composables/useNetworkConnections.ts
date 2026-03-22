import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { NetworkConnection } from "../types/network";

const shouldUseMockConnections = import.meta.env.DEV;

const createConnectionId = () => `webdav-${Date.now()}`;

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

    const discoverConnections = () => {
        if (!shouldUseMockConnections) return 0;
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

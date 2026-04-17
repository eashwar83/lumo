<script setup lang="ts">
import { computed, nextTick, reactive, ref } from "vue";
import type {
    NetworkConnection,
    NetworkFileRow,
    NetworkPlayRequest,
} from "../types/network";
import { useNetworkPanel } from "../composables/useNetworkPanel";
import ConfirmDialog from "../components/ConfirmDialog.vue";
import NetworkConnectionsView from "../components/network/NetworkConnectionsView.vue";
import NetworkBrowserView from "../components/network/NetworkBrowserView.vue";
import NetworkConnectionModal from "../components/network/NetworkConnectionModal.vue";

const {
    viewMode,
    activeConnectionLabel,
    networkConnections,
    networkEntries,
    pathCrumbs,
    parentPath,
    selectedConnection,
    selectedConnectionConfig,
    networkPath,
    isLoading,
    connectionStatus,
    errorMessage,
    openEntry,
    onRefresh,
    onSaveConnection,
    onCreateConnection,
    onDeleteConnection,
    onDiscoverConnections,
    onOpenBrowser,
    onBackToConnections,
    onBrowsePath,
    buildPlayRequest,
    hasFiles,
} = useNetworkPanel();

const emit = defineEmits<{
    (e: "play-network", payload: NetworkPlayRequest): void;
}>();

const isCreateModalOpen = ref(false);
const isCreatingConnection = ref(false);
const createError = ref("");
const isDeleteModalOpen = ref(false);
const isDeletingConnection = ref(false);
const deleteError = ref("");
const pendingDeleteConnection = ref<NetworkConnection | null>(null);
const isSwitchingView = ref(false);
const isDiscovering = ref(false);
const editingConnectionId = ref<string | null>(null);
const canDiscoverConnections = true;

type SupportedProtocol = "webdav" | "smb" | "ftp" | "http-dlna";

type ProtocolOption = {
    value: SupportedProtocol;
    label: string;
};

const protocolOptions: ProtocolOption[] = [
    { value: "webdav", label: "WebDAV" },
    { value: "smb", label: "SMB" },
    { value: "ftp", label: "FTP" },
    { value: "http-dlna", label: "HTTP DLNA" },
];

const createForm = reactive({
    label: "",
    protocol: "webdav" as SupportedProtocol,
    baseUrl: "",
    host: "",
    share: "",
    port: "21",
    username: "",
    password: "",
    defaultPath: "/",
});

const isWebdavProtocol = computed(() => createForm.protocol === "webdav");
const isSmbProtocol = computed(() => createForm.protocol === "smb");
const isFtpProtocol = computed(() => createForm.protocol === "ftp");
const isHttpDlnaProtocol = computed(() => createForm.protocol === "http-dlna");
const requiresAuthFields = computed(
    () =>
        isWebdavProtocol.value || isSmbProtocol.value || isFtpProtocol.value,
);
const serverFieldLabel = computed(() => {
    if (isHttpDlnaProtocol.value) return "Device URL";
    return "Server URL";
});
const serverFieldPlaceholder = computed(() => {
    if (isHttpDlnaProtocol.value) return "http://192.168.31.66:8200/MediaServer";
    return "https://example.com/webdav";
});
const defaultPathLabel = computed(() => {
    if (isHttpDlnaProtocol.value) return "Content Path";
    return "Default Path";
});
const selectedProtocolLabel = computed(
    () =>
        protocolOptions.find((option) => option.value === createForm.protocol)?.label ??
        "WebDAV",
);

const resetCreateForm = () => {
    createForm.label = "";
    createForm.protocol = "webdav";
    createForm.baseUrl = "";
    createForm.host = "";
    createForm.share = "";
    createForm.port = "21";
    createForm.username = "";
    createForm.password = "";
    createForm.defaultPath = "/";
    createError.value = "";
};

const openCreateModal = () => {
    editingConnectionId.value = null;
    resetCreateForm();
    isCreateModalOpen.value = true;
};

const openEditModal = (connection: NetworkConnection) => {
    const protocol = protocolOptions.some(
        (option) => option.value === connection.protocol,
    )
        ? (connection.protocol as SupportedProtocol)
        : "webdav";
    const normalizedBaseUrl = connection.baseUrl || "";
    const normalizedDefaultPath = connection.defaultPath || "/";

    editingConnectionId.value = connection.id;
    createForm.label = connection.label || "WebDAV";
    createForm.protocol = protocol;
    createForm.baseUrl = normalizedBaseUrl;
    createForm.host = "";
    createForm.share = "";
    createForm.port = "21";
    createForm.username = connection.username || "";
    createForm.password = connection.password || "";
    createForm.defaultPath = normalizedDefaultPath;

    if (protocol === "smb") {
        const smbMatch = normalizedBaseUrl.match(/^smb:\/\/([^/]+)\/?(.*)$/i);
        if (smbMatch) {
            createForm.host = smbMatch[1] ?? "";
            const sharePath = smbMatch[2] ?? "";
            createForm.share = sharePath.split("/").filter(Boolean)[0] ?? "";
        } else {
            createForm.host = normalizedBaseUrl;
        }
    }

    if (protocol === "ftp") {
        try {
            const parsed = new URL(normalizedBaseUrl);
            createForm.host = parsed.hostname || "";
            createForm.port = parsed.port || "21";
        } catch {
            createForm.host = normalizedBaseUrl.replace(/^ftp:\/\//i, "");
            createForm.port = "21";
        }
    }

    createError.value = "";
    isCreateModalOpen.value = true;
};

const closeCreateModal = () => {
    if (isCreatingConnection.value) return;
    isCreateModalOpen.value = false;
    editingConnectionId.value = null;
    createError.value = "";
};

const closeDeleteModal = () => {
    if (isDeletingConnection.value) return;
    isDeleteModalOpen.value = false;
    pendingDeleteConnection.value = null;
    deleteError.value = "";
};

const onEntryClick = async (entry: NetworkFileRow) => {
    if (entry.type === "FILE") {
        emit("play-network", buildPlayRequest(entry));
        return;
    }
    if (isLoading.value) return;
    await openEntry(entry);
};

const onOpenConnectionBrowser = async (connectionId: string) => {
    if (isSwitchingView.value) return;
    isSwitchingView.value = true;
    await nextTick();
    try {
        await onOpenBrowser(connectionId);
    } finally {
        isSwitchingView.value = false;
    }
};

const visibleNetworkEntries = computed(() =>
    networkEntries.value.filter((entry) => !entry.isParent),
);

const onBackFolderClick = async () => {
    if (isLoading.value) return;
    if (!parentPath.value) return;
    await onBrowsePath(parentPath.value);
};

const onPathCrumbClick = async (path: string) => {
    if (isLoading.value) return;
    if (path === networkPath.value) return;
    await onBrowsePath(path);
};

const onRefreshClick = async () => {
    await onRefresh();
};

const onConnectionsRefreshClick = async () => {
    if (isDiscovering.value) return;
    isDiscovering.value = true;
    try {
        await onDiscoverConnections();
    } finally {
        isDiscovering.value = false;
    }
};

const shouldShowPathBar = computed(
    () => Boolean(parentPath.value) || pathCrumbs.value.length > 0,
);

const toErrorMessage = (error: unknown, fallback: string) => {
    if (error instanceof Error && error.message) return error.message;
    if (typeof error === "string" && error) return error;
    return fallback;
};

const buildConnectionBaseUrl = () => {
    if (isSmbProtocol.value) {
        const host = createForm.host.trim();
        const share = createForm.share.trim().replace(/^\/+/, "");
        if (!host) {
            throw new Error("SMB host is required");
        }
        if (!share) {
            throw new Error("SMB share is required");
        }
        return `smb://${host}/${share}`;
    }
    if (isFtpProtocol.value) {
        const host = createForm.host.trim();
        const port = createForm.port.trim() || "21";
        if (!host) {
            throw new Error("FTP host is required");
        }
        return `ftp://${host}:${port}`;
    }
    const baseUrl = createForm.baseUrl.trim();
    if (!baseUrl) {
        throw new Error(
            isHttpDlnaProtocol.value
                ? "DLNA device URL is required"
                : "Server URL is required",
        );
    }
    return baseUrl;
};

const buildAutoConnectionLabel = (baseUrl: string) => {
    if (isSmbProtocol.value) {
        const host = createForm.host.trim();
        const share = createForm.share.trim().replace(/^\/+/, "");
        if (host && share) return `${share} @ ${host}`;
        if (host) return host;
        return "Connection";
    }
    if (isFtpProtocol.value) {
        const host = createForm.host.trim();
        return host || "Connection";
    }
    try {
        const parsed = new URL(baseUrl);
        return parsed.hostname || "Connection";
    } catch {
        return "Connection";
    }
};

const onCreateConnectionSubmit = async () => {
    const label = createForm.label.trim();
    const username = requiresAuthFields.value ? createForm.username.trim() : "";
    const defaultPath = createForm.defaultPath.trim() || "/";
    const isEditing = Boolean(editingConnectionId.value);

    isCreatingConnection.value = true;
    createError.value = "";

    try {
        const baseUrl = buildConnectionBaseUrl();
        const normalizedLabel = label || buildAutoConnectionLabel(baseUrl);
        if (isEditing) {
            selectedConnection.value = editingConnectionId.value ?? "";
        } else {
            onCreateConnection();
        }
        const connection = selectedConnectionConfig.value as
            | NetworkConnection
            | null;
        if (!connection) {
            throw new Error("Connection draft is not ready");
        }
        connection.label = normalizedLabel;
        connection.protocol = createForm.protocol;
        connection.baseUrl = baseUrl;
        connection.username = username;
        connection.password = requiresAuthFields.value ? createForm.password : "";
        connection.defaultPath = defaultPath;
        await onSaveConnection();
        isCreateModalOpen.value = false;
        editingConnectionId.value = null;
        resetCreateForm();
    } catch (error) {
        createError.value = toErrorMessage(
            error,
            isEditing ? "Failed to save connection" : "Failed to create connection",
        );
    } finally {
        isCreatingConnection.value = false;
    }
};

const onDeleteConnectionItem = async (connection: NetworkConnection) => {
    pendingDeleteConnection.value = connection;
    deleteError.value = "";
    isDeleteModalOpen.value = true;
};

const onDeleteConnectionConfirm = async () => {
    const connection = pendingDeleteConnection.value;
    if (!connection) return;
    isDeletingConnection.value = true;
    deleteError.value = "";
    try {
        selectedConnection.value = connection.id;
        await onDeleteConnection();
        const stillExists = networkConnections.value.some(
            (item) => item.id === connection.id,
        );
        if (stillExists) {
            deleteError.value = errorMessage.value || "Failed to delete connection";
            return;
        }
        closeDeleteModal();
    } finally {
        isDeletingConnection.value = false;
    }
};

const titleStatusText = computed(() =>
    isLoading.value ? "Loading" : connectionStatus.value,
);

const shouldShowTitleStatus = computed(
    () => titleStatusText.value !== "Connected",
);

const isEditingConnection = computed(() => Boolean(editingConnectionId.value));
const pendingDeleteLabel = computed(
    () =>
        pendingDeleteConnection.value?.label ||
        pendingDeleteConnection.value?.id ||
        "this connection",
);
const deleteConfirmMessage = computed(
    () => `Delete connection "${pendingDeleteLabel.value}"?`,
);

const formatProtocolLabel = (protocol: string) =>
    protocol?.trim() ? protocol.trim().toUpperCase() : "WEBDAV";
</script>

<template>
    <div class="panel panel--network">
        <div
            class="network-body"
        >
            <div class="panel__header network-header">
            <div class="network-header__main">
                <div
                    class="panel__title network-title"
                    :class="{ 'network-title--browser': viewMode === 'browser' }"
                >
                    <template v-if="viewMode === 'connections'">
                        Network
                    </template>
                    <template v-else>
                        <button
                            class="network-icon-btn network-icon-btn--close network-title__home-btn"
                            type="button"
                            aria-label="Home"
                            title="Home"
                            @click="onBackToConnections"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 -960 960 960"
                                fill="#e3e3e3"
                            >
                                <path
                                    d="M240-200h120v-240h240v240h120v-360L480-740 240-560v360Zm-80 80v-480l320-240 320 240v480H520v-240h-80v240H160Zm320-350Z"
                                />
                            </svg>
                        </button>
                        <span class="network-title__label">
                            {{ activeConnectionLabel }}
                        </span>
                        <span
                            v-if="shouldShowTitleStatus"
                            class="network-title__status"
                        >
                            {{ titleStatusText }}
                        </span>
                        <span
                            v-if="errorMessage"
                            class="network-title__error"
                            :title="errorMessage"
                        >
                            {{ errorMessage }}
                        </span>
                    </template>
                </div>
                <div v-if="viewMode === 'connections'" class="network-header__meta">
                    {{ `${networkConnections.length} connections` }}
                </div>
                <div
                    v-else-if="shouldShowPathBar"
                    class="network-title__path"
                    :title="networkPath"
                >
                        <button
                            v-if="parentPath"
                            class="network-title__back-btn"
                            type="button"
                        :disabled="isLoading"
                        aria-label="Go to parent folder"
                        @click="onBackFolderClick"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 -960 960 960"
                                fill="#e3e3e3"
                            >
                                <path
                                    d="m313-440 224 224-57 56-320-320 320-320 57 56-224 224h487v80H313Z"
                                />
                            </svg>
                        </button>
                    <span class="network-title__path-root">/</span>
                    <template
                        v-for="(crumb, index) in pathCrumbs"
                        :key="crumb.path"
                    >
                        <button
                            class="network-title__path-crumb"
                            type="button"
                            :disabled="isLoading || crumb.path === networkPath"
                            @click="onPathCrumbClick(crumb.path)"
                        >
                            {{ crumb.label }}
                        </button>
                        <span
                            v-if="index < pathCrumbs.length - 1"
                            class="network-title__path-sep"
                        >
                            /
                        </span>
                    </template>
                </div>
            </div>
            <div class="network-header__actions">
                <template v-if="viewMode === 'connections'">
                    <button
                        v-if="canDiscoverConnections"
                        class="network-new-btn network-new-btn--refresh"
                        :class="{ 'network-new-btn--spinning': isDiscovering }"
                        type="button"
                        aria-label="Refresh connections"
                        :title="isDiscovering ? 'Refreshing' : 'Refresh'"
                        :disabled="isDiscovering"
                        @click="onConnectionsRefreshClick"
                    >
                        <svg
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        >
                            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
                            <path d="M21 3v6h-6" />
                        </svg>
                    </button>
                    <button
                        class="network-new-btn"
                        type="button"
                        aria-label="Add connection"
                        title="New"
                        @click="openCreateModal"
                    >
                        <svg
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                        >
                            <path
                                d="M12 5v14M5 12h14"
                                stroke-width="2"
                                stroke-linecap="round"
                            />
                        </svg>
                    </button>
                </template>
                <template v-else>
                    <button
                        class="network-icon-btn"
                        type="button"
                        aria-label="Refresh"
                        title="Refresh"
                        @click="onRefreshClick"
                    >
                        <svg
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        >
                            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
                            <path d="M21 3v6h-6" />
                        </svg>
                    </button>
                </template>
            </div>
        </div>

        <transition name="network-view-switch" mode="out-in">
            <NetworkConnectionsView
                v-if="viewMode === 'connections'"
                key="connections"
                :network-connections="networkConnections"
                :selected-connection="selectedConnection"
                :format-protocol-label="formatProtocolLabel"
                @open-browser="onOpenConnectionBrowser"
                @edit="openEditModal"
                @delete="onDeleteConnectionItem"
            />
            <NetworkBrowserView
                v-else
                key="browser"
                :is-loading="isLoading"
                :network-path="networkPath"
                :entries="visibleNetworkEntries"
                :has-files="hasFiles"
                @entry-click="onEntryClick"
            />
        </transition>

        <transition name="network-switch-fade">
            <div
                v-if="isSwitchingView && viewMode === 'connections'"
                class="network-switch-indicator"
                aria-live="polite"
            >
                <div class="network-switch-spinner"></div>
                <div class="network-switch-text">Opening connection...</div>
            </div>
        </transition>
        </div>

        <NetworkConnectionModal
            :open="isCreateModalOpen"
            :is-editing-connection="isEditingConnection"
            :selected-protocol-label="selectedProtocolLabel"
            :protocol-options="protocolOptions"
            :create-form="createForm"
            :is-smb-protocol="isSmbProtocol"
            :is-ftp-protocol="isFtpProtocol"
            :requires-auth-fields="requiresAuthFields"
            :server-field-label="serverFieldLabel"
            :server-field-placeholder="serverFieldPlaceholder"
            :default-path-label="defaultPathLabel"
            :create-error="createError"
            :is-creating-connection="isCreatingConnection"
            @close="closeCreateModal"
            @submit="onCreateConnectionSubmit"
        />
        <ConfirmDialog
            :open="isDeleteModalOpen"
            title="Delete Connection"
            :message="deleteConfirmMessage"
            :confirm-text="isDeletingConnection ? 'Deleting...' : 'Delete'"
            :confirm-loading="isDeletingConnection"
            :error-message="deleteError"
            @cancel="closeDeleteModal"
            @confirm="onDeleteConnectionConfirm"
        />
    </div>
</template>

<style src="../styles/panels.css"></style>
<style src="../styles/network-panel.css"></style>

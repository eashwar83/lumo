import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import {
    defaultSettingGroups,
    LOG_LEVEL_SETTING_LABEL,
    LOG_PATH_SETTING_LABEL,
    SETTINGS_UPDATED_EVENT,
    YTDL_PATH_SETTING_LABEL,
    type SettingGroup,
    type SettingItem,
} from "../mock/settings";
import { applyThemeFromSettingGroups } from "../constants/theme";
import { applyWindowDecorationsFromSettingGroups } from "../constants/windowDecorations";
import {
    applyLoggingSettings,
    applyYtdlSettings,
    createDebouncedUiStateSaver,
    loadUiState,
    openLogDirectory,
} from "./useUiStateStore";

type StoredSettingItem = { label: string; value: string };
type StoredSettingGroup = { title: string; items: StoredSettingItem[] };
type RuntimeVersions = {
    soiaVersion: string;
    mpvVersion?: string | null;
    ffmpegVersion?: string | null;
};

type MediaAssociationStatus = {
    supported: boolean;
    targetBundleId: string;
    isDefaultForAll: boolean;
    missingExtensions: string[];
    checkedExtensions: string[];
};

type MediaAssociationApplyResult = {
    success: boolean;
    failedExtensions: string[];
    status: MediaAssociationStatus;
};

type SetDefaultButtonPhase = "idle" | "checking" | "success" | "failed";
type UpdateButtonPhase =
    | "idle"
    | "checking"
    | "downloading"
    | "installing"
    | "failed";

const normalizeLogLevel = (value: string): string | null => {
    const trimmed = value.trim();
    if (!trimmed) return null;
    const raw = trimmed.includes("=")
        ? trimmed.slice(trimmed.indexOf("=") + 1)
        : trimmed;
    const normalized = raw.toLowerCase();
    if (normalized === "error" || normalized === "err") return "Error";
    if (normalized === "warn" || normalized === "warning") return "Warn";
    if (normalized === "info" || normalized === "v" || normalized === "verbose") {
        return "Info";
    }
    if (normalized === "debug") return "Debug";
    if (normalized === "trace") return "Trace";
    return null;
};

const formatByteSize = (bytes: number): string => {
    if (!Number.isFinite(bytes) || bytes < 0) return "0 B";
    if (bytes < 1024) return `${Math.floor(bytes)} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let value = bytes / 1024;
    let unitIndex = 0;
    while (value >= 1024 && unitIndex < units.length - 1) {
        value /= 1024;
        unitIndex += 1;
    }
    const precision = value >= 10 ? 1 : 2;
    return `${value.toFixed(precision)} ${units[unitIndex]}`;
};

const isUpdateBusyPhase = (phase: UpdateButtonPhase): boolean =>
    phase === "checking" || phase === "downloading" || phase === "installing";

let cachedAvailableUpdateVersion: string | null = null;
let cachedAvailableUpdateResource: Update | null = null;
let updateWorkflowPromise: Promise<void> | null = null;

const sharedHasAvailableUpdate = ref(false);
const sharedAvailableUpdateVersion = ref<string | null>(cachedAvailableUpdateVersion);
const sharedUpdateButtonPhase = ref<UpdateButtonPhase>("idle");
const sharedUpdateProgressTotalBytes = ref<number | null>(null);
const sharedUpdateProgressDownloadedBytes = ref(0);
const sharedUpdateDownloadSpeedBytesPerSec = ref<number | null>(null);
const sharedUpdateLastProgressAtMs = ref<number | null>(null);
const sharedUpdateLastProgressBytes = ref(0);

const cloneSettingGroups = () =>
    defaultSettingGroups.map((group) => ({
        title: group.title,
        items: group.items.map((item) => {
            if (item.type === "select") {
                return {
                    ...item,
                    options: [...item.options],
                };
            }
            return { ...item };
        }),
    }));

const mergeSettingGroups = (stored?: StoredSettingGroup[]): SettingGroup[] => {
    const base = cloneSettingGroups();
    if (!stored?.length) return base;

    base.forEach((group) => {
        const storedGroup = stored.find(
            (candidate) => candidate.title === group.title,
        );
        if (!storedGroup) return;
        group.items.forEach((item) => {
            const storedItem = storedGroup.items.find(
                (candidate) => candidate.label === item.label,
            );
            if (!storedItem?.value) return;
            if (item.type === "select") {
                if (item.options.includes(storedItem.value)) {
                    item.value = storedItem.value;
                } else if (item.label === LOG_LEVEL_SETTING_LABEL) {
                    const normalized = normalizeLogLevel(storedItem.value);
                    if (normalized && item.options.includes(normalized)) {
                        item.value = normalized;
                    }
                }
                return;
            }
            if (item.type === "toggle") {
                if (
                    storedItem.value === item.onValue ||
                    storedItem.value === item.offValue
                ) {
                    item.value = storedItem.value;
                }
                return;
            }
            if (item.type === "slider") {
                const parsed = Number.parseFloat(storedItem.value);
                if (!Number.isNaN(parsed)) {
                    item.value = String(parsed);
                }
                return;
            }
            item.value = storedItem.value;
        });
    });

    return base;
};

const toPersistedGroups = (groups: SettingGroup[]): StoredSettingGroup[] =>
    groups.map((group) => ({
        title: group.title,
        items: group.items.map((item) => ({
            label: item.label,
            value: item.value,
        })),
    }));

const emitSettingsUpdated = (groups: SettingGroup[]) => {
    if (typeof window === "undefined") return;
    window.dispatchEvent(
        new CustomEvent(SETTINGS_UPDATED_EVENT, {
            detail: {
                groups: toPersistedGroups(groups),
            },
        }),
    );
};

const applyVisualSettings = (groups: SettingGroup[]) => {
    const persistedGroups = toPersistedGroups(groups);
    applyThemeFromSettingGroups(persistedGroups);
    void applyWindowDecorationsFromSettingGroups(persistedGroups);
};

export const useSettingsPanel = () => {
    const isMacOS =
        typeof navigator !== "undefined" && /mac|darwin/i.test(navigator.userAgent);
    const settingGroups = ref(cloneSettingGroups());
    const runtimeVersions = ref<RuntimeVersions | null>(null);
    const mediaAssociationStatus = ref<MediaAssociationStatus | null>(null);
    const isCheckingMediaAssociation = ref(false);
    const isApplyingMediaAssociation = ref(false);
    const mediaAssociationMessage = ref("");
    const setDefaultButtonPhase = ref<SetDefaultButtonPhase>("idle");
    const hasAvailableUpdate = sharedHasAvailableUpdate;
    const availableUpdateVersion = sharedAvailableUpdateVersion;
    const updateButtonPhase = sharedUpdateButtonPhase;
    const updateProgressTotalBytes = sharedUpdateProgressTotalBytes;
    const updateProgressDownloadedBytes = sharedUpdateProgressDownloadedBytes;
    const updateDownloadSpeedBytesPerSec = sharedUpdateDownloadSpeedBytesPerSec;
    const updateLastProgressAtMs = sharedUpdateLastProgressAtMs;
    const updateLastProgressBytes = sharedUpdateLastProgressBytes;
    const isLoading = ref(true);
    const uiStateSaver = createDebouncedUiStateSaver(300);
    const LOGGING_APPLY_DELAY_MS = 250;
    const YTDL_APPLY_DELAY_MS = 250;
    let loggingApplyTimer: number | null = null;
    let ytdlApplyTimer: number | null = null;
    let unlistenUpdateAvailable: UnlistenFn | null = null;
    let lastAppliedLoggingRequestKey: string | null = null;
    let lastAppliedYtdlRequestKey: string | null = null;

    const closeUpdateResource = async (update: Update | null) => {
        if (!update) return;
        try {
            await update.close();
        } catch {
            // Ignore cleanup failures from updater resource handles.
        }
    };

    const setAvailableUpdateResource = async (update: Update | null) => {
        if (cachedAvailableUpdateResource && cachedAvailableUpdateResource !== update) {
            await closeUpdateResource(cachedAvailableUpdateResource);
        }
        cachedAvailableUpdateResource = update;
        hasAvailableUpdate.value = Boolean(update);
        availableUpdateVersion.value = update?.version?.trim() || null;
        cachedAvailableUpdateVersion = availableUpdateVersion.value;
    };

    const consumeAvailableUpdateResource = () => {
        const update = cachedAvailableUpdateResource;
        cachedAvailableUpdateResource = null;
        return update;
    };

    const findLogPathItem = (groups: SettingGroup[]) => {
        const item = groups
            .flatMap((group) => group.items)
            .find(
                (candidate) =>
                    candidate.type === "path" &&
                    candidate.label === LOG_PATH_SETTING_LABEL,
            );
        return item?.type === "path" ? item : undefined;
    };

    const isFixedLogPathItem = (item: SettingItem): boolean =>
        item.type === "path" && item.label === LOG_PATH_SETTING_LABEL;

    const findLogLevelItem = (groups: SettingGroup[]) => {
        const item = groups
            .flatMap((group) => group.items)
            .find(
                (candidate) =>
                    candidate.type === "select" &&
                    candidate.label === LOG_LEVEL_SETTING_LABEL,
            );
        return item?.type === "select" ? item : undefined;
    };

    const findYtdlPathItem = (groups: SettingGroup[]) => {
        const item = groups
            .flatMap((group) => group.items)
            .find(
                (candidate) =>
                    candidate.type === "path" &&
                    candidate.label === YTDL_PATH_SETTING_LABEL,
            );
        return item?.type === "path" ? item : undefined;
    };

    const buildLoggingRequestKey = (
        logLevelItem: ReturnType<typeof findLogLevelItem>,
    ): string =>
        JSON.stringify({
            logLevel: logLevelItem?.value.trim() ?? "",
        });

    const applyLoggingOptions = async (groups: SettingGroup[]) => {
        const logPathItem = findLogPathItem(groups);
        const logLevelItem = findLogLevelItem(groups);
        const inputLogLevel = logLevelItem?.value.trim();
        const requestedLogLevel = inputLogLevel ? inputLogLevel : undefined;
        const requestKey = buildLoggingRequestKey(logLevelItem);

        if (requestKey === lastAppliedLoggingRequestKey) {
            return;
        }

        const applied = await applyLoggingSettings(requestedLogLevel);
        if (!applied) return;

        if (logPathItem && applied.logPath && logPathItem.value !== applied.logPath) {
            logPathItem.value = applied.logPath;
        }
        if (
            logLevelItem &&
            applied.logLevel &&
            logLevelItem.value !== applied.logLevel &&
            logLevelItem.options.includes(applied.logLevel)
        ) {
            logLevelItem.value = applied.logLevel;
        }

        lastAppliedLoggingRequestKey = buildLoggingRequestKey(logLevelItem);
    };

    const buildYtdlRequestKey = (
        ytdlPathItem: ReturnType<typeof findYtdlPathItem>,
    ): string =>
        JSON.stringify({
            ytdlPath: ytdlPathItem ? ytdlPathItem.value.trim() : null,
        });

    const applyYtdlOptions = async (groups: SettingGroup[]) => {
        const ytdlPathItem = findYtdlPathItem(groups);
        const requestKey = buildYtdlRequestKey(ytdlPathItem);

        if (requestKey === lastAppliedYtdlRequestKey) {
            return;
        }

        const inputYtdlPath = ytdlPathItem?.value.trim();
        const requestedYtdlPath = inputYtdlPath ? inputYtdlPath : undefined;
        const applied = await applyYtdlSettings(requestedYtdlPath);
        if (!applied) return;

        if (ytdlPathItem) {
            const appliedYtdlPath = applied.ytdlPath ?? "";
            if (ytdlPathItem.value !== appliedYtdlPath) {
                ytdlPathItem.value = appliedYtdlPath;
            }
        }

        lastAppliedYtdlRequestKey = buildYtdlRequestKey(ytdlPathItem);
    };

    const scheduleApplyLoggingOptions = () => {
        if (loggingApplyTimer) {
            window.clearTimeout(loggingApplyTimer);
        }
        loggingApplyTimer = window.setTimeout(() => {
            void applyLoggingOptions(settingGroups.value);
            loggingApplyTimer = null;
        }, LOGGING_APPLY_DELAY_MS);
    };

    const scheduleApplyYtdlOptions = () => {
        if (ytdlApplyTimer) {
            window.clearTimeout(ytdlApplyTimer);
        }
        ytdlApplyTimer = window.setTimeout(() => {
            void applyYtdlOptions(settingGroups.value);
            ytdlApplyTimer = null;
        }, YTDL_APPLY_DELAY_MS);
    };

    const initializeLoggingOptions = async (groups: SettingGroup[]) => {
        await Promise.all([applyLoggingOptions(groups), applyYtdlOptions(groups)]);
    };

    const resetAllSettings = () => {
        settingGroups.value = cloneSettingGroups();
        void initializeLoggingOptions(settingGroups.value);
        applyVisualSettings(settingGroups.value);
    };

    const browseForPath = async (item: SettingItem) => {
        if (item.type !== "path") return;
        if (isFixedLogPathItem(item)) {
            try {
                await openLogDirectory();
            } catch {
                // Intentionally ignore open-directory failures in the panel flow.
            }
            return;
        }
        const selected = await open({
            multiple: false,
            directory: false,
            title: item.browseTitle ?? "Select file",
        });
        if (selected) {
            item.value = selected as string;
        }
    };

    const saveState = () => {
        uiStateSaver.saveDebounced({
            settings: {
                groups: toPersistedGroups(settingGroups.value),
            },
        });
    };

    const loadState = async () => {
        const stored = await loadUiState<{
            settings?: { groups?: StoredSettingGroup[] };
        }>();
        const groups = mergeSettingGroups(stored?.settings?.groups);
        await initializeLoggingOptions(groups);
        settingGroups.value = groups;
        applyVisualSettings(settingGroups.value);
        isLoading.value = false;
    };

    const loadRuntimeVersions = async () => {
        try {
            runtimeVersions.value = await invoke<RuntimeVersions>(
                "get_runtime_versions",
            );
        } catch {
            runtimeVersions.value = null;
        }
    };

    const loadUpdateAvailability = async () => {
        try {
            hasAvailableUpdate.value = await invoke<boolean>("has_available_update");
            if (hasAvailableUpdate.value) {
                availableUpdateVersion.value = cachedAvailableUpdateVersion;
                if (!cachedAvailableUpdateVersion || !cachedAvailableUpdateResource) {
                    void resolveAvailableUpdateVersion();
                }
            } else {
                void setAvailableUpdateResource(null);
            }
        } catch {
            void setAvailableUpdateResource(null);
        }
    };

    const resolveAvailableUpdateVersion = async () => {
        try {
            const update = await check();
            await setAvailableUpdateResource(update);
        } catch {
            // Keep last known availability when refresh check fails.
        }
    };

    const setupUpdateAvailabilityListener = async () => {
        try {
            unlistenUpdateAvailable = await listen<boolean>(
                "soia-update-available",
                (event) => {
                    hasAvailableUpdate.value = event.payload === true;
                    if (hasAvailableUpdate.value) {
                        availableUpdateVersion.value = cachedAvailableUpdateVersion;
                        if (!cachedAvailableUpdateVersion || !cachedAvailableUpdateResource) {
                            void resolveAvailableUpdateVersion();
                        }
                    } else {
                        void setAvailableUpdateResource(null);
                    }
                    if (!hasAvailableUpdate.value && !isUpdateBusyPhase(updateButtonPhase.value)) {
                        updateButtonPhase.value = "idle";
                    }
                },
            );
        } catch {
            unlistenUpdateAvailable = null;
        }
    };

    const refreshMediaAssociationStatus = async () => {
        isCheckingMediaAssociation.value = true;
        mediaAssociationMessage.value = "";
        try {
            mediaAssociationStatus.value = await invoke<MediaAssociationStatus>(
                "get_media_association_status",
            );
        } catch {
            mediaAssociationStatus.value = null;
            mediaAssociationMessage.value =
                "Unable to read current default app associations.";
        } finally {
            isCheckingMediaAssociation.value = false;
        }
    };

    const setMediaAssociationToSoia = async () => {
        if (setDefaultButtonPhase.value === "checking") return;
        isApplyingMediaAssociation.value = true;
        setDefaultButtonPhase.value = "checking";
        mediaAssociationMessage.value = "";
        try {
            const result = await invoke<MediaAssociationApplyResult>(
                "set_media_association_to_soia",
            );
            mediaAssociationStatus.value = result.status;
            if (result.status.isDefaultForAll) {
                setDefaultButtonPhase.value = "success";
                mediaAssociationMessage.value =
                    "Soia is now the default app for checked media extensions.";
            } else if (result.failedExtensions.length) {
                setDefaultButtonPhase.value = "failed";
                mediaAssociationMessage.value =
                    "Some extensions could not be updated automatically.";
            } else {
                setDefaultButtonPhase.value = "failed";
                mediaAssociationMessage.value =
                    "Default app was not updated for all checked extensions.";
            }
        } catch {
            setDefaultButtonPhase.value = "failed";
            mediaAssociationMessage.value =
                "Failed to set Soia as the default media app.";
        } finally {
            isApplyingMediaAssociation.value = false;
        }
    };

    const canManageMediaAssociation = computed(() => isMacOS);

    const shouldShowSetDefaultMediaButton = computed(() => {
        const status = mediaAssociationStatus.value;
        const needsAction = Boolean(
            isMacOS && status?.supported && !status.isDefaultForAll,
        );
        return (
            needsAction ||
            setDefaultButtonPhase.value !== "idle"
        );
    });

    const isSetDefaultButtonDisabled = computed(
        () =>
            setDefaultButtonPhase.value === "checking" ||
            setDefaultButtonPhase.value === "success" ||
            setDefaultButtonPhase.value === "failed",
    );

    const isSetDefaultButtonLoading = computed(
        () => setDefaultButtonPhase.value === "checking",
    );

    const setDefaultButtonText = computed(() => {
        if (setDefaultButtonPhase.value === "success") {
            return "Success";
        }
        if (setDefaultButtonPhase.value === "failed") {
            return "Failed";
        }
        return "Set as default";
    });
    const isSetDefaultSuccess = computed(
        () => setDefaultButtonPhase.value === "success",
    );

    const shouldShowUpdateButton = computed(
        () =>
            hasAvailableUpdate.value ||
            isUpdateBusyPhase(updateButtonPhase.value) ||
            updateButtonPhase.value === "failed",
    );

    const isUpdateButtonDisabled = computed(
        () => isUpdateBusyPhase(updateButtonPhase.value),
    );

    const updateButtonText = computed(() => {
        if (
            updateButtonPhase.value === "checking" ||
            updateButtonPhase.value === "downloading"
        ) {
            return "Updating...";
        }
        if (updateButtonPhase.value === "installing") {
            return "Installing...";
        }
        if (updateButtonPhase.value === "failed") {
            return "Retry";
        }
        return "Update";
    });
    const isUpdateRetry = computed(
        () => updateButtonPhase.value === "failed",
    );

    const shouldShowUpdateStatus = computed(
        () =>
            updateButtonPhase.value === "checking" ||
            updateButtonPhase.value === "downloading" ||
            updateButtonPhase.value === "installing",
    );

    const updateDownloadPercent = computed(() => {
        if (updateButtonPhase.value !== "downloading") return null;
        const total = updateProgressTotalBytes.value;
        if (!total || total <= 0) return null;
        const ratio = updateProgressDownloadedBytes.value / total;
        const percent = Math.round(Math.min(1, Math.max(0, ratio)) * 100);
        return Number.isFinite(percent) ? percent : null;
    });

    const updateStatusText = computed(() => {
        if (updateButtonPhase.value === "checking") {
            return "Checking for update... 0%";
        }
        if (updateButtonPhase.value === "downloading") {
            const percent = updateDownloadPercent.value;
            const speed = updateDownloadSpeedBytesPerSec.value;
            const speedText =
                typeof speed === "number" && Number.isFinite(speed) && speed > 0
                    ? ` · ${formatByteSize(speed)}/s`
                    : "";
            if (typeof percent === "number") {
                return `Downloading update... ${percent}%${speedText}`;
            }
            return `Downloading update...${speedText}`;
        }
        if (updateButtonPhase.value === "installing") {
            return "Installing update...";
        }
        return "";
    });

    const shouldShowUpdateHint = computed(
        () => hasAvailableUpdate.value && !isUpdateBusyPhase(updateButtonPhase.value),
    );

    const updateHintText = computed(() => {
        const version = availableUpdateVersion.value?.trim();
        if (version) {
            return `Version ${version} is available!`;
        }
        return "New version is available!";
    });

    const installUpdate = async () => {
        if (isUpdateBusyPhase(updateButtonPhase.value)) return;
        if (updateWorkflowPromise) {
            await updateWorkflowPromise;
            return;
        }

        updateWorkflowPromise = (async () => {
        updateProgressTotalBytes.value = null;
        updateProgressDownloadedBytes.value = 0;
        updateDownloadSpeedBytesPerSec.value = null;
        updateLastProgressAtMs.value = null;
        updateLastProgressBytes.value = 0;

        const runDownloadAndInstall = async (update: Update) => {
            hasAvailableUpdate.value = true;
            availableUpdateVersion.value = update.version?.trim() || null;
            updateButtonPhase.value = "downloading";
            await update.download((event: DownloadEvent) => {
                if (event.event === "Started") {
                    const contentLength = event.data.contentLength;
                    if (
                        typeof contentLength === "number" &&
                        Number.isFinite(contentLength) &&
                        contentLength > 0
                    ) {
                        updateProgressTotalBytes.value = contentLength;
                    } else {
                        updateProgressTotalBytes.value = null;
                    }
                    updateProgressDownloadedBytes.value = 0;
                    updateDownloadSpeedBytesPerSec.value = null;
                    updateLastProgressAtMs.value = Date.now();
                    updateLastProgressBytes.value = 0;
                    return;
                }
                if (event.event === "Progress") {
                    const chunkLength = event.data.chunkLength;
                    if (
                        typeof chunkLength === "number" &&
                        Number.isFinite(chunkLength) &&
                        chunkLength > 0
                    ) {
                        const nextValue = updateProgressDownloadedBytes.value + chunkLength;
                        const total = updateProgressTotalBytes.value;
                        updateProgressDownloadedBytes.value =
                            total && total > 0 ? Math.min(nextValue, total) : nextValue;
                    }

                    const now = Date.now();
                    const lastAt = updateLastProgressAtMs.value;
                    const lastBytes = updateLastProgressBytes.value;
                    if (lastAt && now > lastAt) {
                        const bytesDelta = updateProgressDownloadedBytes.value - lastBytes;
                        const timeDeltaSec = (now - lastAt) / 1000;
                        if (bytesDelta > 0 && timeDeltaSec > 0) {
                            const instantSpeed = bytesDelta / timeDeltaSec;
                            const previous = updateDownloadSpeedBytesPerSec.value;
                            updateDownloadSpeedBytesPerSec.value =
                                previous && Number.isFinite(previous)
                                    ? previous * 0.7 + instantSpeed * 0.3
                                    : instantSpeed;
                        }
                    }
                    updateLastProgressAtMs.value = now;
                    updateLastProgressBytes.value = updateProgressDownloadedBytes.value;
                    return;
                }
                if (event.event === "Finished") {
                    const total = updateProgressTotalBytes.value;
                    if (total && total > 0) {
                        updateProgressDownloadedBytes.value = total;
                    }
                    updateDownloadSpeedBytesPerSec.value = null;
                }
            });
            updateButtonPhase.value = "installing";
            await update.install();
            try {
                const shouldRelaunch = await confirm(
                    "Update installed. Restart now to apply the new version?",
                    {
                        title: "Restart Required",
                        kind: "info",
                        okLabel: "Restart now",
                        cancelLabel: "Later",
                    },
                );
                if (shouldRelaunch) {
                    await relaunch();
                }
            } catch {
                // Update is already installed; avoid surfacing this as update failure.
            }
        };

        let primaryUpdate = consumeAvailableUpdateResource();
        let lastKnownVersion: string | null = primaryUpdate?.version?.trim() || null;

        try {
            if (!primaryUpdate) {
                updateButtonPhase.value = "checking";
                primaryUpdate = await check();
            }
            if (!primaryUpdate) {
                await setAvailableUpdateResource(null);
                updateButtonPhase.value = "idle";
                return;
            }

            lastKnownVersion = primaryUpdate.version?.trim() || lastKnownVersion;

            try {
                await runDownloadAndInstall(primaryUpdate);
                closeUpdateResource(primaryUpdate);
                await setAvailableUpdateResource(null);
                updateButtonPhase.value = "idle";
                return;
            } catch {
                closeUpdateResource(primaryUpdate);
                updateButtonPhase.value = "checking";
                const fallbackUpdate = await check();
                if (!fallbackUpdate) {
                    await setAvailableUpdateResource(null);
                    updateButtonPhase.value = "idle";
                    return;
                }
                lastKnownVersion = fallbackUpdate.version?.trim() || lastKnownVersion;
                try {
                    await runDownloadAndInstall(fallbackUpdate);
                    closeUpdateResource(fallbackUpdate);
                    await setAvailableUpdateResource(null);
                    updateButtonPhase.value = "idle";
                    return;
                } catch {
                    closeUpdateResource(fallbackUpdate);
                    hasAvailableUpdate.value = true;
                    availableUpdateVersion.value = lastKnownVersion;
                    updateButtonPhase.value = "failed";
                    return;
                }
            }
        } catch {
            hasAvailableUpdate.value = true;
            availableUpdateVersion.value = lastKnownVersion;
            updateButtonPhase.value = "failed";
        }
        })();

        try {
            await updateWorkflowPromise;
        } finally {
            updateWorkflowPromise = null;
        }
    };

    const mediaAssociationSummary = computed(() => {
        if (!isMacOS) {
            return "Default media app detection is only available on macOS.";
        }
        const status = mediaAssociationStatus.value;
        if (!status) {
            return "Unable to detect current default app associations.";
        }
        if (!status.supported) {
            return "Default media app detection is unavailable on this system.";
        }
        if (status.isDefaultForAll) {
            return "Soia is already the default app for checked media extensions.";
        }
        if (!status.missingExtensions.length) {
            return "Some media extensions are not currently handled by Soia.";
        }
        const previewLimit = 8;
        const preview = status.missingExtensions.slice(0, previewLimit).join(", ");
        const remaining = status.missingExtensions.length - previewLimit;
        if (remaining > 0) {
            return `Not default for: ${preview}, +${remaining} more.`;
        }
        return `Not default for: ${preview}.`;
    });

    onMounted(() => {
        void loadState();
        void loadRuntimeVersions();
        void refreshMediaAssociationStatus();
        void loadUpdateAvailability();
        void setupUpdateAvailabilityListener();
    });

    onUnmounted(() => {
        if (loggingApplyTimer) {
            window.clearTimeout(loggingApplyTimer);
            loggingApplyTimer = null;
        }
        if (ytdlApplyTimer) {
            window.clearTimeout(ytdlApplyTimer);
            ytdlApplyTimer = null;
        }
        unlistenUpdateAvailable?.();
        unlistenUpdateAvailable = null;
    });

    watch(
        settingGroups,
        () => {
            if (isLoading.value) return;
            applyVisualSettings(settingGroups.value);
            scheduleApplyLoggingOptions();
            scheduleApplyYtdlOptions();
            saveState();
            emitSettingsUpdated(settingGroups.value);
        },
        { deep: true },
    );

    return {
        settingGroups,
        runtimeVersions,
        mediaAssociationStatus,
        canManageMediaAssociation,
        shouldShowSetDefaultMediaButton,
        isSetDefaultButtonDisabled,
        isSetDefaultButtonLoading,
        setDefaultButtonText,
        isSetDefaultSuccess,
        shouldShowUpdateButton,
        isUpdateButtonDisabled,
        updateButtonText,
        isUpdateRetry,
        shouldShowUpdateStatus,
        updateStatusText,
        shouldShowUpdateHint,
        updateHintText,
        mediaAssociationSummary,
        mediaAssociationMessage,
        isCheckingMediaAssociation,
        isApplyingMediaAssociation,
        refreshMediaAssociationStatus,
        setMediaAssociationToSoia,
        installUpdate,
        resetAllSettings,
        browseForPath,
        isFixedLogPathItem,
        isLoading,
    };
};

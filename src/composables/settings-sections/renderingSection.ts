import { open } from "@tauri-apps/plugin-dialog";
import { ref } from "vue";
import {
    applyRenderingSettings,
    pickPathsNative,
    resolveExistingShaderFiles,
    resolveShaderCandidates,
} from "../useUiStateStore";

export type StoredRenderingState = {
    renderingMode?: "normal" | "animeMode" | "animeAuto";
    selectedShaderFiles?: string[];
    activeShaderFiles?: string[];
    animeModeEnabled?: boolean;
    animeAutoShaderEnabled?: boolean;
    // Legacy split-selected fields kept for migration only.
    normalSelectedShaderFiles?: string[];
    normalActiveShaderFiles?: string[];
    animeModeSelectedShaderFiles?: string[];
    animeModeActiveShaderFiles?: string[];
    animeSelectedShaderFiles?: string[];
    animeActiveShaderFiles?: string[];
};

export const useRenderingSettingsSection = () => {
    const selectedShaderFiles = ref<string[]>([]);
    const activeShaderFiles = ref<string[]>([]);
    const unavailableShaderFiles = ref<string[]>([]);
    const multiShaderEnabled = ref(false);
    const renderingMode = ref<"normal" | "animeMode">("normal");
    const normalActiveShaderFiles = ref<string[]>([]);
    const animeModeActiveShaderFiles = ref<string[]>([]);

    const RENDERING_APPLY_DELAY_MS = 200;
    let renderingApplyTimer: number | null = null;
    let lastAppliedRenderingRequestKey: string | null = null;
    let shaderAvailabilityRequestId = 0;

    const normalizeShaderFiles = (input: string[]): string[] => {
        const normalized: string[] = [];
        input.forEach((value) => {
            const trimmed = value.trim();
            if (!trimmed) return;
            if (!/\.glsl$/i.test(trimmed)) return;
            if (!normalized.includes(trimmed)) {
                normalized.push(trimmed);
            }
        });
        return normalized;
    };

    const normalizeActiveShaderFiles = (
        selectedFiles: string[],
        activeFiles: string[],
    ): string[] => {
        const normalized = normalizeShaderFiles(activeFiles).filter((file) =>
            selectedFiles.includes(file),
        );
        return normalized;
    };

    const persistCurrentModeSelection = () => {
        if (renderingMode.value === "normal") {
            normalActiveShaderFiles.value = [...activeShaderFiles.value];
            return;
        }
        animeModeActiveShaderFiles.value = [...activeShaderFiles.value];
    };

    const hydrateFromModeSelection = () => {
        if (renderingMode.value === "normal") {
            activeShaderFiles.value = [...normalActiveShaderFiles.value];
            return;
        }
        activeShaderFiles.value = [...animeModeActiveShaderFiles.value];
    };

    const ensureValidShaderSelection = () => {
        selectedShaderFiles.value = normalizeShaderFiles(selectedShaderFiles.value);
        activeShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            activeShaderFiles.value,
        );
        normalActiveShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            normalActiveShaderFiles.value,
        );
        animeModeActiveShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            animeModeActiveShaderFiles.value,
        );
        if (selectedShaderFiles.value.length <= 1) {
            multiShaderEnabled.value = false;
        }
        if (!multiShaderEnabled.value && activeShaderFiles.value.length > 1) {
            activeShaderFiles.value = activeShaderFiles.value.slice(0, 1);
        }
        unavailableShaderFiles.value = unavailableShaderFiles.value.filter((file) =>
            selectedShaderFiles.value.includes(file),
        );
    };

    const applyUnavailableFiltering = () => {
        if (!unavailableShaderFiles.value.length) return;
        const unavailableSet = new Set(unavailableShaderFiles.value);
        activeShaderFiles.value = activeShaderFiles.value.filter(
            (file) => !unavailableSet.has(file),
        );
    };

    const refreshShaderAvailability = async () => {
        const snapshot = [...selectedShaderFiles.value];
        if (!snapshot.length) {
            unavailableShaderFiles.value = [];
            return;
        }
        const requestId = ++shaderAvailabilityRequestId;
        const existing = await resolveExistingShaderFiles(snapshot);
        if (requestId !== shaderAvailabilityRequestId) return;
        const existingSet = new Set(existing);
        unavailableShaderFiles.value = snapshot.filter(
            (file) => !existingSet.has(file),
        );
        applyUnavailableFiltering();
    };

    const buildRenderingRequestKey = (): string =>
        JSON.stringify({
            renderingMode: renderingMode.value,
            selectedShaderFiles: selectedShaderFiles.value,
            activeShaderFiles: activeShaderFiles.value,
        });

    const applyRenderingOptions = async () => {
        ensureValidShaderSelection();
        applyUnavailableFiltering();
        persistCurrentModeSelection();
        const runtimeActiveShaderFiles =
            renderingMode.value === "normal" ? activeShaderFiles.value : [];
        const requestKey = buildRenderingRequestKey();
        if (requestKey === lastAppliedRenderingRequestKey) {
            return;
        }

        const applied = await applyRenderingSettings(
            selectedShaderFiles.value,
            runtimeActiveShaderFiles,
        );
        if (!applied) return;

        selectedShaderFiles.value = normalizeShaderFiles(applied.selectedShaderFiles);
        if (renderingMode.value === "normal") {
            activeShaderFiles.value = normalizeActiveShaderFiles(
                selectedShaderFiles.value,
                applied.activeShaderFiles,
            );
        } else {
            activeShaderFiles.value = normalizeActiveShaderFiles(
                selectedShaderFiles.value,
                activeShaderFiles.value,
            );
        }
        applyUnavailableFiltering();
        persistCurrentModeSelection();

        lastAppliedRenderingRequestKey = buildRenderingRequestKey();
    };

    const scheduleApplyRenderingOptions = () => {
        if (renderingApplyTimer) {
            window.clearTimeout(renderingApplyTimer);
        }
        renderingApplyTimer = window.setTimeout(() => {
            void applyRenderingOptions();
            renderingApplyTimer = null;
        }, RENDERING_APPLY_DELAY_MS);
    };

    const browseForCustomShaders = async () => {
        let paths: string[] = [];
        const nativeSelected = await pickPathsNative();
        if (Array.isArray(nativeSelected)) {
            paths = nativeSelected;
        } else {
            const selected = await open({
                multiple: true,
                directory: false,
                title: "Select shader files or folders",
            });
            if (!selected) return;
            paths = Array.isArray(selected) ? selected : [selected];
        }

        const expanded = await resolveShaderCandidates(paths);
        const normalized = normalizeShaderFiles(expanded);
        if (!normalized.length) return;

        const next = [...selectedShaderFiles.value];
        normalized.forEach((path) => {
            if (!next.includes(path)) {
                next.push(path);
            }
        });
        selectedShaderFiles.value = next;
        activeShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            activeShaderFiles.value,
        );
        persistCurrentModeSelection();
        await refreshShaderAvailability();
    };

    const setShaderEnabled = (shaderFile: string, enabled: boolean) => {
        if (!selectedShaderFiles.value.includes(shaderFile)) return;
        if (unavailableShaderFiles.value.includes(shaderFile)) return;
        if (enabled) {
            if (!multiShaderEnabled.value) {
                activeShaderFiles.value = [shaderFile];
            } else if (!activeShaderFiles.value.includes(shaderFile)) {
                activeShaderFiles.value = [...activeShaderFiles.value, shaderFile];
            }
        } else {
            activeShaderFiles.value = activeShaderFiles.value.filter(
                (path) => path !== shaderFile,
            );
        }
        activeShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            activeShaderFiles.value,
        );
        persistCurrentModeSelection();
    };

    const setMultiShaderEnabled = (enabled: boolean) => {
        multiShaderEnabled.value = enabled && selectedShaderFiles.value.length > 1;
        if (!multiShaderEnabled.value && activeShaderFiles.value.length > 1) {
            activeShaderFiles.value = activeShaderFiles.value.slice(0, 1);
        }
        persistCurrentModeSelection();
    };

    const removeShaderFromList = (shaderFile: string) => {
        selectedShaderFiles.value = selectedShaderFiles.value.filter(
            (path) => path !== shaderFile,
        );
        activeShaderFiles.value = activeShaderFiles.value.filter(
            (path) => path !== shaderFile,
        );
        unavailableShaderFiles.value = unavailableShaderFiles.value.filter(
            (path) => path !== shaderFile,
        );
        if (selectedShaderFiles.value.length <= 1) {
            multiShaderEnabled.value = false;
        }
        persistCurrentModeSelection();
    };

    const clearShaders = () => {
        selectedShaderFiles.value = [];
        activeShaderFiles.value = [];
        unavailableShaderFiles.value = [];
        multiShaderEnabled.value = false;
        persistCurrentModeSelection();
    };

    const setRenderingMode = (mode: "normal" | "animeMode") => {
        if (mode === renderingMode.value) return;
        persistCurrentModeSelection();
        renderingMode.value = mode;
        hydrateFromModeSelection();
        ensureValidShaderSelection();
        void refreshShaderAvailability();
        scheduleApplyRenderingOptions();
    };

    const resetRenderingSettings = () => {
        selectedShaderFiles.value = [];
        activeShaderFiles.value = [];
        multiShaderEnabled.value = false;
        renderingMode.value = "normal";
        normalActiveShaderFiles.value = [];
        animeModeActiveShaderFiles.value = [];
        void applyRenderingOptions();
    };

    const loadRenderingSettings = async (stored?: StoredRenderingState) => {
        const legacyMode: "normal" | "animeMode" =
            stored?.renderingMode === "animeMode" ||
            stored?.renderingMode === "animeAuto" ||
            stored?.animeModeEnabled === true ||
            stored?.animeAutoShaderEnabled === true
                ? "animeMode"
                : "normal";

        const legacySelected = normalizeShaderFiles(
            stored?.selectedShaderFiles ?? [],
        );
        const legacyActive = normalizeActiveShaderFiles(
            legacySelected,
            stored?.activeShaderFiles ?? [],
        );

        selectedShaderFiles.value = legacySelected;
        if (!selectedShaderFiles.value.length) {
            selectedShaderFiles.value = normalizeShaderFiles([
                ...(stored?.normalSelectedShaderFiles ?? []),
                ...(stored?.animeModeSelectedShaderFiles ?? []),
                ...(stored?.animeSelectedShaderFiles ?? []),
            ]);
        }
        normalActiveShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            stored?.normalActiveShaderFiles ??
                (legacyMode === "normal" ? legacyActive : []),
        );

        animeModeActiveShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            stored?.animeModeActiveShaderFiles ??
                stored?.animeActiveShaderFiles ??
                (legacyMode === "animeMode" ? legacyActive : []),
        );

        renderingMode.value = legacyMode;
        hydrateFromModeSelection();
        multiShaderEnabled.value =
            selectedShaderFiles.value.length > 1 &&
            activeShaderFiles.value.length > 1;
        await refreshShaderAvailability();
        await applyRenderingOptions();
    };

    const dispose = () => {
        if (renderingApplyTimer) {
            window.clearTimeout(renderingApplyTimer);
            renderingApplyTimer = null;
        }
    };

    return {
        selectedShaderFiles,
        activeShaderFiles,
        unavailableShaderFiles,
        multiShaderEnabled,
        renderingMode,
        browseForCustomShaders,
        setShaderEnabled,
        setMultiShaderEnabled,
        setRenderingMode,
        removeShaderFromList,
        clearShaders,
        scheduleApplyRenderingOptions,
        resetRenderingSettings,
        loadRenderingSettings,
        dispose,
        toPersistedRendering: () => ({
            renderingMode: renderingMode.value,
            selectedShaderFiles: selectedShaderFiles.value,
            normalActiveShaderFiles:
                renderingMode.value === "normal"
                    ? activeShaderFiles.value
                    : normalActiveShaderFiles.value,
            activeShaderFiles: activeShaderFiles.value,
            animeModeActiveShaderFiles:
                renderingMode.value === "animeMode"
                    ? activeShaderFiles.value
                    : animeModeActiveShaderFiles.value,
            animeModeEnabled: renderingMode.value === "animeMode",
        }),
    };
};

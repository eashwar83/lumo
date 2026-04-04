import { open } from "@tauri-apps/plugin-dialog";
import { ref } from "vue";
import {
    applyRenderingSettings,
    pickPathsNative,
    resolveExistingShaderFiles,
    resolveShaderCandidates,
} from "../useUiStateStore";

export type StoredRenderingState = {
    selectedShaderFiles?: string[];
    activeShaderFiles?: string[];
};

export const useRenderingSettingsSection = () => {
    const selectedShaderFiles = ref<string[]>([]);
    const activeShaderFiles = ref<string[]>([]);
    const unavailableShaderFiles = ref<string[]>([]);

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

    const ensureValidShaderSelection = () => {
        selectedShaderFiles.value = normalizeShaderFiles(selectedShaderFiles.value);
        activeShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            activeShaderFiles.value,
        );
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
            selectedShaderFiles: selectedShaderFiles.value,
            activeShaderFiles: activeShaderFiles.value,
        });

    const applyRenderingOptions = async () => {
        ensureValidShaderSelection();
        applyUnavailableFiltering();
        const requestKey = buildRenderingRequestKey();
        if (requestKey === lastAppliedRenderingRequestKey) {
            return;
        }

        const applied = await applyRenderingSettings(
            selectedShaderFiles.value,
            activeShaderFiles.value,
        );
        if (!applied) return;

        selectedShaderFiles.value = normalizeShaderFiles(applied.selectedShaderFiles);
        activeShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            applied.activeShaderFiles,
        );
        applyUnavailableFiltering();

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
        await refreshShaderAvailability();
    };

    const setShaderEnabled = (shaderFile: string, enabled: boolean) => {
        if (!selectedShaderFiles.value.includes(shaderFile)) return;
        if (unavailableShaderFiles.value.includes(shaderFile)) return;
        if (enabled) {
            if (!activeShaderFiles.value.includes(shaderFile)) {
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
    };

    const clearShaders = () => {
        selectedShaderFiles.value = [];
        activeShaderFiles.value = [];
        unavailableShaderFiles.value = [];
    };

    const resetRenderingSettings = () => {
        selectedShaderFiles.value = [];
        activeShaderFiles.value = [];
        void applyRenderingOptions();
    };

    const loadRenderingSettings = async (stored?: StoredRenderingState) => {
        selectedShaderFiles.value = normalizeShaderFiles(
            stored?.selectedShaderFiles ?? [],
        );
        activeShaderFiles.value = normalizeActiveShaderFiles(
            selectedShaderFiles.value,
            stored?.activeShaderFiles ?? [],
        );
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
        browseForCustomShaders,
        setShaderEnabled,
        removeShaderFromList,
        clearShaders,
        scheduleApplyRenderingOptions,
        resetRenderingSettings,
        loadRenderingSettings,
        dispose,
        toPersistedRendering: () => ({
            selectedShaderFiles: selectedShaderFiles.value,
            activeShaderFiles: activeShaderFiles.value,
        }),
    };
};

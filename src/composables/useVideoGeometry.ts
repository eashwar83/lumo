import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";

// Per-file video geometry: the aspect-ratio override and the auto-crop rect are
// remembered per file (keyed by media URL/path) and restored on reopen. Both
// persist in the `perFileVideo` ui-state slice.

export type PerFileVideo = {
    aspect?: string;
    crop?: string;
    fitWindow?: boolean;
};

type StoredPerFileVideo = Record<string, PerFileVideo>;

// mpv `video-aspect-override` values. "-1" restores the container's own aspect.
const ASPECT_PRESETS: { label: string; value: string }[] = [
    { label: "Default", value: "-1" },
    { label: "16:9", value: "16:9" },
    { label: "4:3", value: "4:3" },
    { label: "21:9", value: "21:9" },
    { label: "2.35:1", value: "2.35" },
];

const MAX_ENTRIES = 300;

export const useVideoGeometry = () => {
    const entries = new Map<string, PerFileVideo>();
    const persistedSaver = createDebouncedUiStateSaver(300);
    const currentAspectLabel = ref("Default");

    let currentKey = "";

    const normalizeKey = (key: string) => key.trim();

    const setProp = async (name: string, value: string) => {
        try {
            await invoke("mpv_set_option_string", { name, value });
        } catch (error) {
            console.warn("[geometry] set option failed", { name, value, error });
        }
    };

    const persist = () => {
        const object: StoredPerFileVideo = {};
        entries.forEach((value, key) => {
            object[key] = value;
        });
        persistedSaver.saveDebounced({ perFileVideo: object });
    };

    // Re-insert to keep insertion order (newest last) and evict the oldest.
    const touch = (key: string, next: PerFileVideo) => {
        entries.delete(key);
        entries.set(key, next);
        while (entries.size > MAX_ENTRIES) {
            const oldest = entries.keys().next().value;
            if (oldest === undefined) break;
            entries.delete(oldest);
        }
    };

    const aspectIndexForValue = (value?: string): number => {
        if (!value) return 0;
        const index = ASPECT_PRESETS.findIndex((preset) => preset.value === value);
        return index >= 0 ? index : 0;
    };

    const applyAspectForMedia = async (key: string) => {
        currentKey = normalizeKey(key);
        const saved = entries.get(currentKey)?.aspect;
        const index = aspectIndexForValue(saved);
        currentAspectLabel.value = ASPECT_PRESETS[index].label;
        // Always set the override explicitly so a previous file's aspect never
        // leaks into a file that has no saved preference ("-1" = container).
        await setProp("video-aspect-override", ASPECT_PRESETS[index].value);
    };

    // Advance to the next aspect preset, apply it, remember it. Returns the OSD
    // label for the caller to surface.
    const cycleAspect = async (): Promise<string> => {
        const current = aspectIndexForValue(entries.get(currentKey)?.aspect);
        const next = (current + 1) % ASPECT_PRESETS.length;
        const preset = ASPECT_PRESETS[next];
        currentAspectLabel.value = preset.label;
        await setProp("video-aspect-override", preset.value);
        if (currentKey) {
            const existing = entries.get(currentKey) ?? {};
            // "Default" is the natural state — store undefined so we don't keep
            // a redundant entry around.
            touch(currentKey, {
                ...existing,
                aspect: preset.value === "-1" ? undefined : preset.value,
            });
            persist();
        }
        return preset.label;
    };

    const getCrop = (key: string): string | null => {
        const value = entries.get(normalizeKey(key))?.crop;
        return value && value.trim() ? value : null;
    };

    const setCrop = (key: string, value: string) => {
        const normalizedKey = normalizeKey(key);
        if (!normalizedKey) return;
        const existing = entries.get(normalizedKey) ?? {};
        const crop = value.trim() ? value : undefined;
        // Drop the whole entry if nothing is worth remembering.
        if (!crop && existing.aspect === undefined && !existing.fitWindow) {
            entries.delete(normalizedKey);
        } else {
            touch(normalizedKey, { ...existing, crop });
        }
        persist();
    };

    const isFitWindow = (key: string): boolean =>
        entries.get(normalizeKey(key))?.fitWindow === true;

    const setFitWindow = (key: string, value: boolean) => {
        const normalizedKey = normalizeKey(key);
        if (!normalizedKey) return;
        const existing = entries.get(normalizedKey) ?? {};
        const fitWindow = value ? true : undefined;
        if (!fitWindow && existing.aspect === undefined && !existing.crop) {
            entries.delete(normalizedKey);
        } else {
            touch(normalizedKey, { ...existing, fitWindow });
        }
        persist();
    };

    const load = (stored?: StoredPerFileVideo) => {
        if (!stored) return;
        Object.entries(stored).forEach(([key, value]) => {
            const normalizedKey = normalizeKey(key);
            if (!normalizedKey) return;
            entries.set(normalizedKey, {
                aspect: value.aspect,
                crop: value.crop,
                fitWindow: value.fitWindow,
            });
        });
    };

    onMounted(async () => {
        const state = await loadUiState<{ perFileVideo?: StoredPerFileVideo }>();
        load(state?.perFileVideo);
    });

    return {
        currentAspectLabel,
        applyAspectForMedia,
        cycleAspect,
        getCrop,
        setCrop,
        isFitWindow,
        setFitWindow,
    };
};

export type VideoGeometryController = ReturnType<typeof useVideoGeometry>;

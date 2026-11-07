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
export const ASPECT_PRESETS: { label: string; value: string }[] = [
    { label: "Default", value: "-1" },
    { label: "16:9", value: "16:9" },
    { label: "4:3", value: "4:3" },
    { label: "21:9", value: "21:9" },
    { label: "2.35:1", value: "2.35" },
];

export type ParsedAspectRatio = {
    /** Value passed to mpv `video-aspect-override` and ffmpeg `convert_file`. */
    value: string;
    /** Human-readable label shown in menus and OSD. */
    label: string;
    /** Numeric width / height ratio (0 for Default). */
    numericRatio: number;
};

const cleanNumber = (n: number): string =>
    Number.isInteger(n) ? String(n) : String(Math.round(n * 1000) / 1000);

/**
 * Parses a preset or custom aspect ratio string (e.g. "16:10", "16/10",
 * "1.85:1", "2.39", "9:16", "-1", "Default") into a normalised value + label.
 * Returns null when the input is not a valid positive aspect ratio.
 */
export const parseCustomAspectRatio = (raw: string): ParsedAspectRatio | null => {
    const trimmed = raw.trim();
    if (!trimmed) return null;
    const lower = trimmed.toLowerCase();
    if (lower === "-1" || lower === "default" || lower === "keep" || lower === "original") {
        return { value: "-1", label: "Default", numericRatio: 0 };
    }

    const presetDirect = ASPECT_PRESETS.find(
        (p) => p.value.toLowerCase() === lower || p.label.toLowerCase() === lower,
    );
    if (presetDirect) {
        if (presetDirect.value === "-1") {
            return { value: "-1", label: "Default", numericRatio: 0 };
        }
        const parts = presetDirect.value.split(":");
        const ratio =
            parts.length === 2
                ? Number(parts[0]) / Number(parts[1])
                : Number(presetDirect.value);
        return {
            value: presetDirect.value,
            label: presetDirect.label,
            numericRatio: ratio,
        };
    }

    const sepMatch = trimmed.match(/^([0-9]+(?:\.[0-9]+)?)\s*[:/]\s*([0-9]+(?:\.[0-9]+)?)$/);
    if (sepMatch) {
        const w = Number.parseFloat(sepMatch[1]);
        const h = Number.parseFloat(sepMatch[2]);
        if (!Number.isFinite(w) || !Number.isFinite(h) || w <= 0 || h <= 0) {
            return null;
        }
        const ratio = w / h;
        if (ratio < 0.1 || ratio > 10) return null;
        const wStr = cleanNumber(w);
        const hStr = cleanNumber(h);
        const candidateColon = `${wStr}:${hStr}`;
        const presetMatch = ASPECT_PRESETS.find(
            (p) =>
                p.value === candidateColon ||
                p.label === candidateColon ||
                (h === 1 && p.value === wStr),
        );
        if (presetMatch) {
            return {
                value: presetMatch.value,
                label: presetMatch.label,
                numericRatio: ratio,
            };
        }
        if (h === 1 && !Number.isInteger(w)) {
            return {
                value: wStr,
                label: `${wStr}:1`,
                numericRatio: ratio,
            };
        }
        return {
            value: candidateColon,
            label: candidateColon,
            numericRatio: ratio,
        };
    }

    if (/^[0-9]+(?:\.[0-9]+)?$/.test(trimmed)) {
        const val = Number.parseFloat(trimmed);
        if (!Number.isFinite(val) || val < 0.1 || val > 10) return null;
        const valStr = cleanNumber(val);
        const presetMatch = ASPECT_PRESETS.find((p) => p.value === valStr);
        if (presetMatch) {
            return {
                value: presetMatch.value,
                label: presetMatch.label,
                numericRatio: val,
            };
        }
        return {
            value: valStr,
            label: `${valStr}:1`,
            numericRatio: val,
        };
    }

    return null;
};

const MAX_ENTRIES = 300;

export const useVideoGeometry = () => {
    const entries = new Map<string, PerFileVideo>();
    const persistedSaver = createDebouncedUiStateSaver(300);
    const currentAspectValue = ref("-1");
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

    const resolveSavedAspect = (value?: string): { value: string; label: string } => {
        if (!value) return ASPECT_PRESETS[0];
        const parsed = parseCustomAspectRatio(value);
        return parsed ? { value: parsed.value, label: parsed.label } : ASPECT_PRESETS[0];
    };

    const applyAspectForMedia = async (key: string) => {
        currentKey = normalizeKey(key);
        const saved = entries.get(currentKey)?.aspect;
        const resolved = resolveSavedAspect(saved);
        currentAspectValue.value = resolved.value;
        currentAspectLabel.value = resolved.label;
        // Always set the override explicitly so a previous file's aspect never
        // leaks into a file that has no saved preference ("-1" = container).
        await setProp("video-aspect-override", resolved.value);
    };

    // Apply an explicit preset or custom aspect ratio, remember it per-file, and
    // return the OSD label for the caller to surface.
    const setAspect = async (rawValue: string): Promise<string> => {
        const resolved = resolveSavedAspect(rawValue);
        currentAspectValue.value = resolved.value;
        currentAspectLabel.value = resolved.label;
        await setProp("video-aspect-override", resolved.value);
        if (currentKey) {
            const existing = entries.get(currentKey) ?? {};
            const nextAspect = resolved.value === "-1" ? undefined : resolved.value;
            if (!nextAspect && !existing.crop && !existing.fitWindow) {
                entries.delete(currentKey);
            } else {
                touch(currentKey, {
                    ...existing,
                    aspect: nextAspect,
                });
            }
            persist();
        }
        return resolved.label;
    };

    // Advance to the next aspect preset, apply it, remember it. Returns the OSD
    // label for the caller to surface.
    const cycleAspect = async (): Promise<string> => {
        const saved = entries.get(currentKey)?.aspect ?? currentAspectValue.value;
        const currentIndex = ASPECT_PRESETS.findIndex((preset) => preset.value === saved);
        const next = currentIndex >= 0 ? (currentIndex + 1) % ASPECT_PRESETS.length : 0;
        const preset = ASPECT_PRESETS[next];
        return setAspect(preset.value);
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
        currentAspectValue,
        currentAspectLabel,
        applyAspectForMedia,
        setAspect,
        cycleAspect,
        getCrop,
        setCrop,
        isFitWindow,
        setFitWindow,
    };
};

export type VideoGeometryController = ReturnType<typeof useVideoGeometry>;

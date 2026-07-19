import { computed, onMounted, ref } from "vue";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";

// A video "look" preset: a named bundle of the 10 colour parameters. Applying
// one sets them all; saving one snapshots the current values. Built-in presets
// ship with the app; custom presets are user-saved and persisted globally (a
// reusable library, applied to any video's per-file look).

export type PresetValues = {
    brightness: number;
    contrast: number;
    saturation: number;
    gamma: number;
    hue: number;
    exposure: number;
    temperature: number;
    tint: number;
    highlights: number;
    shadows: number;
};

export type VideoPreset = {
    id: string;
    name: string;
    values: PresetValues;
    builtIn?: boolean;
};

const NEUTRAL: PresetValues = {
    brightness: 0,
    contrast: 0,
    saturation: 0,
    gamma: 0,
    hue: 0,
    exposure: 0,
    temperature: 0,
    tint: 0,
    highlights: 0,
    shadows: 0,
};

const preset = (
    id: string,
    name: string,
    values: Partial<PresetValues>,
): VideoPreset => ({
    id,
    name,
    builtIn: true,
    values: { ...NEUTRAL, ...values },
});

// Curated built-in looks. Values are in the -100..100 slider space.
export const BUILT_IN_PRESETS: VideoPreset[] = [
    preset("punch", "Punch", {
        contrast: 30,
        saturation: 25,
        exposure: 4,
        highlights: -12,
        shadows: 6,
    }),
    preset("warm", "Warm", { temperature: 35, contrast: 12, saturation: 10 }),
    preset("cool", "Cool", { temperature: -35, contrast: 10, saturation: 8 }),
    preset("vivid", "Vivid", { saturation: 40, contrast: 20, exposure: 3 }),
    preset("calm", "Calm", {
        contrast: -10,
        saturation: -8,
        exposure: 3,
        temperature: 8,
        highlights: -10,
        shadows: 12,
    }),
    preset("bw", "B&W", { saturation: -100, contrast: 15 }),
    preset("bw-warm", "B&W Warm", {
        saturation: -100,
        temperature: 30,
        contrast: 12,
    }),
    preset("vintage", "Vintage", {
        saturation: -20,
        temperature: 25,
        contrast: -8,
        highlights: -15,
        shadows: 12,
        tint: 6,
    }),
];

type VideoPresetsOptions = {
    applyValues: (values: PresetValues) => void | Promise<void>;
    readCurrentValues: () => PresetValues;
    onApplied?: (name: string) => void;
};

const clamp = (value: number) =>
    Math.min(100, Math.max(-100, Math.round(value)));

const normalizeValues = (values?: Partial<PresetValues>): PresetValues => {
    const out = { ...NEUTRAL };
    (Object.keys(NEUTRAL) as (keyof PresetValues)[]).forEach((key) => {
        const v = values?.[key];
        if (typeof v === "number" && Number.isFinite(v)) out[key] = clamp(v);
    });
    return out;
};

export const useVideoPresets = (options: VideoPresetsOptions) => {
    const customPresets = ref<VideoPreset[]>([]);
    const saver = createDebouncedUiStateSaver(300);

    const allPresets = computed<VideoPreset[]>(() => [
        ...BUILT_IN_PRESETS,
        ...customPresets.value,
    ]);

    const persist = () => {
        saver.saveDebounced({
            customVideoPresets: customPresets.value.map((p) => ({
                id: p.id,
                name: p.name,
                values: p.values,
            })),
        });
    };

    const apply = async (item: VideoPreset) => {
        await options.applyValues({ ...item.values });
        options.onApplied?.(item.name);
    };

    // Deterministic-enough id without Date.now/Math.random (unavailable here).
    let seq = 0;
    const nextId = () => {
        seq += 1;
        return `custom-${customPresets.value.length}-${seq}`;
    };

    const saveCurrent = (rawName: string) => {
        const name = rawName.trim();
        if (!name) return;
        customPresets.value = [
            ...customPresets.value,
            {
                id: nextId(),
                name,
                values: normalizeValues(options.readCurrentValues()),
            },
        ];
        persist();
    };

    const remove = (id: string) => {
        customPresets.value = customPresets.value.filter((p) => p.id !== id);
        persist();
    };

    onMounted(async () => {
        const stored = await loadUiState<{
            customVideoPresets?: Array<{
                id?: string;
                name?: string;
                values?: Partial<PresetValues>;
            }>;
        }>();
        const list = stored?.customVideoPresets;
        if (Array.isArray(list)) {
            customPresets.value = list
                .filter((p) => p && typeof p.name === "string" && p.name.trim())
                .map((p, index) => ({
                    id: p.id?.trim() || `custom-restored-${index}`,
                    name: (p.name as string).trim(),
                    values: normalizeValues(p.values),
                }));
        }
    });

    return {
        allPresets,
        customPresets,
        apply,
        saveCurrent,
        remove,
    };
};

export type VideoPresetsController = ReturnType<typeof useVideoPresets>;

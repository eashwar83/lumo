import { onMounted, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";

// Video-quality enhancements driven entirely through the existing mpv bridge
// (`mpv_set_option_string` for GPU/renderer properties, `mpv_run_command` for
// the video-filter chain). No new backend command is required.
//
//  - Quality preset  -> gpu scaler / deband / dither properties
//  - Sharpness       -> `unsharp` video filter (Amount + Radius, USM-style)
//  - Denoise         -> `hqdn3d` video filter
//  - Deinterlace     -> mpv's built-in `deinterlace` property
//
// The two lavfi video filters (unsharp, hqdn3d) don't work with non-copy
// hardware decoding, so while either is active we flip hwdec to `auto-copy`
// (mirroring the approach in useAutoCrop) and restore `auto` when both are off.

export type QualityPreset = "fast" | "balanced" | "high";
export type AiUpscaleMode = "off" | "anime" | "live";

export type StoredVideoEnhancements = {
    qualityPreset?: QualityPreset;
    sharpenAmount?: number;
    sharpenRadius?: number;
    aiUpscale?: AiUpscaleMode;
};

export type VideoEnhancementsState = {
    qualityPreset: QualityPreset;
    /** 0 = off .. 100 = strongest (maps to unsharp luma amount 0..2). */
    sharpenAmount: number;
    /** Unsharp matrix size in px (odd, 3..15). */
    sharpenRadius: number;
    denoise: boolean;
    deinterlace: boolean;
    aiUpscale: AiUpscaleMode;
};

// mpv gpu renderer properties per preset. All are runtime-changeable, so the
// switch is instant and applies to the frame on screen.
const QUALITY_PRESETS: Record<QualityPreset, Record<string, string>> = {
    fast: {
        scale: "bilinear",
        cscale: "bilinear",
        dscale: "bilinear",
        deband: "no",
        "correct-downscaling": "no",
        "linear-downscaling": "no",
        "sigmoid-upscaling": "no",
        "dither-depth": "no",
    },
    balanced: {
        scale: "spline36",
        cscale: "spline36",
        dscale: "mitchell",
        deband: "yes",
        "correct-downscaling": "yes",
        "linear-downscaling": "no",
        "sigmoid-upscaling": "no",
        "dither-depth": "auto",
    },
    high: {
        scale: "ewa_lanczossharp",
        cscale: "ewa_lanczossharp",
        dscale: "mitchell",
        deband: "yes",
        "correct-downscaling": "yes",
        "linear-downscaling": "yes",
        "sigmoid-upscaling": "yes",
        "dither-depth": "auto",
    },
};

const SHARPEN_LABEL = "lumo-sharpen";
const DENOISE_LABEL = "lumo-denoise";

const DEFAULT_STATE: VideoEnhancementsState = {
    qualityPreset: "balanced",
    sharpenAmount: 0,
    sharpenRadius: 5,
    denoise: false,
    deinterlace: false,
    aiUpscale: "off",
};

const clamp = (value: number, min: number, max: number) =>
    Math.min(max, Math.max(min, value));

// Unsharp needs an odd matrix size; snap and clamp into a sane range.
const normalizeRadius = (value: number): number => {
    const rounded = clamp(Math.round(value), 3, 15);
    return rounded % 2 === 0 ? rounded + 1 : rounded;
};

const isQualityPreset = (value: unknown): value is QualityPreset =>
    value === "fast" || value === "balanced" || value === "high";

const isAiUpscaleMode = (value: unknown): value is AiUpscaleMode =>
    value === "off" || value === "anime" || value === "live";

export const useVideoEnhancements = () => {
    const state = reactive<VideoEnhancementsState>({ ...DEFAULT_STATE });
    const persistedSaver = createDebouncedUiStateSaver(300);

    // Track the last hwdec we asked for so we don't reinit the decoder needlessly.
    let lastHwdec: "auto" | "auto-copy" = "auto";
    let loaded = false;

    const setProp = async (name: string, value: string) => {
        try {
            await invoke("mpv_set_option_string", { name, value });
        } catch (error) {
            console.warn("[enhance] set option failed", { name, value, error });
        }
    };

    const runCommand = async (args: Array<string | number>) => {
        try {
            await invoke("mpv_run_command", { args });
        } catch (error) {
            // `vf remove` on an absent label errors harmlessly — swallow it.
            console.warn("[enhance] mpv command failed", { args, error });
        }
    };

    const persist = () => {
        persistedSaver.saveDebounced({
            videoEnhancements: {
                qualityPreset: state.qualityPreset,
                sharpenAmount: state.sharpenAmount,
                sharpenRadius: state.sharpenRadius,
                aiUpscale: state.aiUpscale,
            },
        });
    };

    // --- mpv application ----------------------------------------------------

    const applyQualityPreset = async () => {
        const config = QUALITY_PRESETS[state.qualityPreset];
        await Promise.all(
            Object.entries(config).map(([name, value]) => setProp(name, value)),
        );
    };

    const buildUnsharp = (): string => {
        const amount = clamp(state.sharpenAmount, 0, 100) / 50; // 0 .. 2.0
        const size = normalizeRadius(state.sharpenRadius);
        // luma_x:luma_y:luma_amount:chroma_x:chroma_y:chroma_amount (luma only)
        return `unsharp=${size}:${size}:${amount.toFixed(3)}:3:3:0`;
    };

    const cpuFiltersActive = () => state.sharpenAmount > 0 || state.denoise;

    const syncHwdec = async () => {
        const want = cpuFiltersActive() ? "auto-copy" : "auto";
        if (want === lastHwdec) return;
        await runCommand(["set", "hwdec", want]);
        lastHwdec = want;
    };

    // Rebuild our labelled video filters deterministically (denoise before
    // sharpen) so toggling one never disturbs the other or the ordering.
    const rebuildVideoFilters = async () => {
        await runCommand(["vf", "remove", `@${DENOISE_LABEL}`]);
        await runCommand(["vf", "remove", `@${SHARPEN_LABEL}`]);
        if (state.denoise) {
            await runCommand(["vf", "add", `@${DENOISE_LABEL}:hqdn3d`]);
        }
        if (state.sharpenAmount > 0) {
            await runCommand(["vf", "add", `@${SHARPEN_LABEL}:${buildUnsharp()}`]);
        }
        await syncHwdec();
    };

    const applyDeinterlace = async () => {
        await setProp("deinterlace", state.deinterlace ? "yes" : "no");
    };

    // Bundled GLSL upscalers, composed with any manual shaders in the backend.
    const applyAiUpscale = async () => {
        try {
            await invoke("apply_upscale_shaders", { mode: state.aiUpscale });
        } catch (error) {
            console.warn("[enhance] apply upscale failed", error);
        }
    };

    // --- public setters -----------------------------------------------------

    const setQualityPreset = async (preset: QualityPreset) => {
        state.qualityPreset = preset;
        await applyQualityPreset();
        persist();
    };

    const setSharpenAmount = async (value: number) => {
        state.sharpenAmount = clamp(Math.round(value), 0, 100);
        await rebuildVideoFilters();
        persist();
    };

    const setSharpenRadius = async (value: number) => {
        state.sharpenRadius = normalizeRadius(value);
        if (state.sharpenAmount > 0) await rebuildVideoFilters();
        persist();
    };

    const setDenoise = async (enabled: boolean) => {
        state.denoise = enabled;
        await rebuildVideoFilters();
    };

    const setDeinterlace = async (enabled: boolean) => {
        state.deinterlace = enabled;
        await applyDeinterlace();
    };

    const setAiUpscale = async (mode: AiUpscaleMode) => {
        state.aiUpscale = mode;
        await applyAiUpscale();
        persist();
    };

    // --- lifecycle ----------------------------------------------------------

    // Reapply everything to the freshly-loaded file. Renderer properties and
    // the vf chain generally persist across files, but reapplying is cheap and
    // guarantees a consistent picture.
    const onFileLoaded = async () => {
        if (!loaded) return;
        await applyQualityPreset();
        await rebuildVideoFilters();
        await applyDeinterlace();
        if (state.aiUpscale !== "off") await applyAiUpscale();
    };

    const load = async (stored?: StoredVideoEnhancements) => {
        if (stored) {
            if (isQualityPreset(stored.qualityPreset)) {
                state.qualityPreset = stored.qualityPreset;
            }
            if (typeof stored.sharpenAmount === "number") {
                state.sharpenAmount = clamp(Math.round(stored.sharpenAmount), 0, 100);
            }
            if (typeof stored.sharpenRadius === "number") {
                state.sharpenRadius = normalizeRadius(stored.sharpenRadius);
            }
            if (isAiUpscaleMode(stored.aiUpscale)) {
                state.aiUpscale = stored.aiUpscale;
            }
        }
        loaded = true;
        // Denoise / deinterlace intentionally default off each launch.
        await applyQualityPreset();
        if (state.aiUpscale !== "off") await applyAiUpscale();
    };

    const reset = async () => {
        Object.assign(state, DEFAULT_STATE);
        await applyQualityPreset();
        await rebuildVideoFilters();
        await applyDeinterlace();
        await applyAiUpscale();
        persist();
    };

    onMounted(async () => {
        const stored = await loadUiState<{
            videoEnhancements?: StoredVideoEnhancements;
        }>();
        await load(stored?.videoEnhancements);
    });

    return {
        state,
        setQualityPreset,
        setSharpenAmount,
        setSharpenRadius,
        setDenoise,
        setDeinterlace,
        setAiUpscale,
        onFileLoaded,
        reset,
    };
};

export type VideoEnhancementsController = ReturnType<typeof useVideoEnhancements>;

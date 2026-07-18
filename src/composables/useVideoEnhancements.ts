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

// Colour-grade parameters, all -100..100 (0 = neutral). Applied via a GPU shader.
export type ColorGradeKey =
    | "exposure"
    | "temperature"
    | "tint"
    | "highlights"
    | "shadows";

export const COLOR_GRADE_KEYS: ColorGradeKey[] = [
    "exposure",
    "temperature",
    "tint",
    "highlights",
    "shadows",
];

export type StoredVideoEnhancements = {
    qualityPreset?: QualityPreset;
    sharpenAmount?: number;
    sharpenRadius?: number;
    aiUpscale?: AiUpscaleMode;
    exposure?: number;
    temperature?: number;
    tint?: number;
    highlights?: number;
    shadows?: number;
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
    exposure: number;
    temperature: number;
    tint: number;
    highlights: number;
    shadows: number;
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

const DENOISE_LABEL = "lumo-denoise";

const DEFAULT_STATE: VideoEnhancementsState = {
    qualityPreset: "balanced",
    sharpenAmount: 0,
    sharpenRadius: 5,
    denoise: false,
    deinterlace: false,
    aiUpscale: "off",
    exposure: 0,
    temperature: 0,
    tint: 0,
    highlights: 0,
    shadows: 0,
};

const clamp = (value: number, min: number, max: number) =>
    Math.min(max, Math.max(min, value));

// Sharpening runs as a GPU unsharp-mask shader, so the radius is a blur
// distance in source pixels and is unbounded (no ffmpeg matrix cap). Small =
// crisp edges, large = broad local-contrast / "HDR"-style glow.
const MAX_SHARPEN_RADIUS = 30;
const normalizeRadius = (value: number): number =>
    clamp(Math.round(value), 1, MAX_SHARPEN_RADIUS);

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
    let onMessage: ((text: string) => void) | null = null;
    const setMessageHandler = (handler: (text: string) => void) => {
        onMessage = handler;
    };

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
                exposure: state.exposure,
                temperature: state.temperature,
                tint: state.tint,
                highlights: state.highlights,
                shadows: state.shadows,
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

    // GPU unsharp shader (no CPU copy-back). Amount 0..100 -> shader amount
    // 0..2.0; radius is a pixel blur distance. amount<=0 clears the shader.
    const applySharpen = async () => {
        const amount = (clamp(state.sharpenAmount, 0, 100) / 100) * 2.0;
        const radius = normalizeRadius(state.sharpenRadius);
        try {
            await invoke("apply_sharpen_shader", { amount, radius });
        } catch (error) {
            console.warn("[enhance] apply sharpen failed", error);
        }
    };

    // Regenerating + reloading the shader on every slider tick is wasteful, so
    // coalesce rapid changes.
    let sharpenTimer: number | null = null;
    const scheduleSharpen = () => {
        if (sharpenTimer) window.clearTimeout(sharpenTimer);
        sharpenTimer = window.setTimeout(() => {
            sharpenTimer = null;
            void applySharpen();
        }, 120);
    };

    // GPU colour-grade shader (exposure / temperature / tint / highlights /
    // shadows). All-zero clears it on the backend.
    const applyColorGrade = async () => {
        try {
            await invoke("apply_color_grade_shader", {
                exposure: clamp(state.exposure, -100, 100),
                temperature: clamp(state.temperature, -100, 100),
                tint: clamp(state.tint, -100, 100),
                highlights: clamp(state.highlights, -100, 100),
                shadows: clamp(state.shadows, -100, 100),
            });
        } catch (error) {
            console.warn("[enhance] apply colour grade failed", error);
        }
    };

    let gradeTimer: number | null = null;
    const scheduleColorGrade = () => {
        if (gradeTimer) window.clearTimeout(gradeTimer);
        gradeTimer = window.setTimeout(() => {
            gradeTimer = null;
            void applyColorGrade();
        }, 120);
    };

    const setColorGrade = (key: ColorGradeKey, value: number) => {
        state[key] = clamp(Math.round(value), -100, 100);
        scheduleColorGrade();
        persist();
    };

    const hasColorGrade = () =>
        COLOR_GRADE_KEYS.some((key) => state[key] !== 0);

    // Only the denoise (hqdn3d) lavfi filter needs software frames now, so hwdec
    // copy-back is tied to denoise alone; sharpening is GPU and stays hw-decoded.
    const syncHwdec = async () => {
        const want = state.denoise ? "auto-copy" : "auto";
        if (want === lastHwdec) return;
        await runCommand(["set", "hwdec", want]);
        lastHwdec = want;
    };

    // hwdec must be switched to a copy mode BEFORE inserting the lavfi filter —
    // it won't attach under non-copy hardware decoding (silently no-ops).
    const rebuildVideoFilters = async () => {
        await runCommand(["vf", "remove", `@${DENOISE_LABEL}`]);
        await syncHwdec();
        if (state.denoise) {
            await runCommand(["vf", "add", `@${DENOISE_LABEL}:hqdn3d`]);
        }
    };

    const applyDeinterlace = async () => {
        await setProp("deinterlace", state.deinterlace ? "yes" : "no");
    };

    // Bundled GLSL upscalers, composed with any manual shaders in the backend.
    // Returns how many shader files actually resolved (0 => resources missing).
    const applyAiUpscale = async (): Promise<number> => {
        try {
            const count = await invoke<number>("apply_upscale_shaders", {
                mode: state.aiUpscale,
            });
            return typeof count === "number" ? count : 0;
        } catch (error) {
            console.warn("[enhance] apply upscale failed", error);
            return 0;
        }
    };

    // --- public setters -----------------------------------------------------

    const setQualityPreset = async (preset: QualityPreset) => {
        state.qualityPreset = preset;
        await applyQualityPreset();
        persist();
    };

    const setSharpenAmount = (value: number) => {
        state.sharpenAmount = clamp(Math.round(value), 0, 100);
        scheduleSharpen();
        persist();
    };

    const setSharpenRadius = (value: number) => {
        state.sharpenRadius = normalizeRadius(value);
        scheduleSharpen();
        persist();
    };

    const setDenoise = async (enabled: boolean) => {
        state.denoise = enabled;
        await rebuildVideoFilters();
        onMessage?.(`Denoise ${enabled ? "on" : "off"}`);
    };

    const setDeinterlace = async (enabled: boolean) => {
        state.deinterlace = enabled;
        await applyDeinterlace();
        onMessage?.(`Deinterlace ${enabled ? "on" : "off"}`);
    };

    const AI_LABELS: Record<AiUpscaleMode, string> = {
        off: "Off",
        anime: "Anime",
        live: "Live-action",
    };

    const setAiUpscale = async (mode: AiUpscaleMode) => {
        state.aiUpscale = mode;
        const count = await applyAiUpscale();
        persist();
        if (mode === "off") {
            onMessage?.("AI Upscale off");
        } else if (count > 0) {
            onMessage?.(`AI Upscale · ${AI_LABELS[mode]} (${count} shaders)`);
        } else {
            onMessage?.(`AI Upscale · ${AI_LABELS[mode]}: shaders not found`);
        }
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
        if (state.sharpenAmount > 0) await applySharpen();
        if (hasColorGrade()) await applyColorGrade();
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
            COLOR_GRADE_KEYS.forEach((key) => {
                const value = stored[key];
                if (typeof value === "number") {
                    state[key] = clamp(Math.round(value), -100, 100);
                }
            });
        }
        loaded = true;
        // Denoise / deinterlace intentionally default off each launch.
        await applyQualityPreset();
        if (state.sharpenAmount > 0) await applySharpen();
        if (hasColorGrade()) await applyColorGrade();
        if (state.aiUpscale !== "off") await applyAiUpscale();
    };

    const reset = async () => {
        Object.assign(state, DEFAULT_STATE);
        await applyQualityPreset();
        await rebuildVideoFilters();
        await applyDeinterlace();
        await applySharpen();
        await applyColorGrade();
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
        setColorGrade,
        setMessageHandler,
        onFileLoaded,
        reset,
    };
};

export type VideoEnhancementsController = ReturnType<typeof useVideoEnhancements>;

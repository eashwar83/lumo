import { computed, onMounted, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";

// Audio processing, driven entirely through mpv's audio-filter chain (`af
// add`/`af remove` with labels) plus lavfi. No new backend command is needed.
//
//   Night mode      -> acompressor + alimiter  (tames explosions, lifts dialogue)
//   Dialogue boost  -> presence-band peaking EQ + a touch of mud cut
//   Graphic EQ      -> 10 chained `equalizer` biquads
//   Gain            -> `volume` boost above the 100% slider, with a limiter
//
// Filters are labelled so each can be toggled independently, and the whole
// chain is rebuilt in a fixed order (EQ -> dialogue -> night -> gain) whenever
// anything changes. Every apply is idempotent and clears itself when neutral.

export type NightModeLevel = "off" | "light" | "medium" | "strong";

export const EQ_BAND_COUNT = 10;
/** ISO octave centres for the graphic EQ, in Hz. */
export const EQ_FREQUENCIES = [
    31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000,
] as const;
/** Short labels for the band sliders. */
export const EQ_BAND_LABELS = [
    "31",
    "62",
    "125",
    "250",
    "500",
    "1k",
    "2k",
    "4k",
    "8k",
    "16k",
] as const;

export const EQ_GAIN_RANGE = 12; // dB, +/-

export type AudioEnhancementsState = {
    nightMode: NightModeLevel;
    /** 0 = off .. 100 = strongest presence lift. */
    dialogueBoost: number;
    /** Post-volume gain in percent, 100 = unity, up to 300. */
    gain: number;
    eqEnabled: boolean;
    /** Per-band gain in dB, -12..+12. Always EQ_BAND_COUNT long. */
    eqBands: number[];
    /** Id of the EQ preset currently applied, or "" once hand-edited. */
    eqPreset: string;
};

export type StoredAudioEnhancements = {
    nightMode?: string;
    dialogueBoost?: number;
    gain?: number;
    eqEnabled?: boolean;
    eqBands?: number[];
    eqPreset?: string;
};

export type EqPreset = { id: string; name: string; bands: number[] };

// Band order matches EQ_FREQUENCIES: 31 62 125 250 500 1k 2k 4k 8k 16k
export const EQ_PRESETS: EqPreset[] = [
    { id: "flat", name: "Flat", bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    { id: "movie", name: "Movie", bands: [3, 2, 0, -1, 0, 2, 3, 2, 1, 0] },
    { id: "dialogue", name: "Dialogue", bands: [-4, -3, -2, -1, 1, 3, 4, 3, 1, 0] },
    { id: "bass", name: "Bass Boost", bands: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0] },
    { id: "treble", name: "Treble Boost", bands: [0, 0, 0, 0, 0, 1, 2, 4, 5, 6] },
    { id: "loudness", name: "Loudness", bands: [5, 4, 2, 0, -1, 0, 1, 3, 5, 5] },
    { id: "rock", name: "Rock", bands: [5, 4, 2, -1, -2, 0, 2, 4, 5, 5] },
    { id: "pop", name: "Pop", bands: [-1, 0, 2, 4, 4, 2, 0, -1, -1, -1] },
    { id: "jazz", name: "Jazz", bands: [3, 2, 1, 2, -1, -1, 0, 1, 2, 3] },
    { id: "classical", name: "Classical", bands: [4, 3, 2, 0, -1, -1, 0, 2, 3, 4] },
    { id: "vocal", name: "Vocal", bands: [-3, -3, -1, 2, 4, 4, 3, 1, 0, -1] },
    { id: "podcast", name: "Podcast", bands: [-6, -5, -2, 1, 3, 3, 2, 1, -1, -3] },
];

// mpv filter labels — one per stage so they can be toggled independently.
const LABEL_EQ = "lumo-eq";
const LABEL_DIALOGUE = "lumo-dialogue";
const LABEL_NIGHT = "lumo-night";
const LABEL_GAIN = "lumo-gain";
const ALL_LABELS = [LABEL_EQ, LABEL_DIALOGUE, LABEL_NIGHT, LABEL_GAIN];

// Night mode = downward compression with make-up gain, then a brick-wall
// limiter so the make-up can't clip. Thresholds are linear amplitude
// (acompressor takes 0.000976..1, i.e. roughly -60..0 dB).
const NIGHT_MODE_GRAPHS: Record<Exclude<NightModeLevel, "off">, string> = {
    // -18 dB / 3:1 — gentle, keeps most of the dynamics.
    light: "acompressor=threshold=0.126:ratio=3:attack=20:release=300:makeup=1.6",
    // -24 dB / 5:1 — the usual "watch it at night" setting.
    medium: "acompressor=threshold=0.063:ratio=5:attack=15:release=250:makeup=2.2",
    // -30 dB / 8:1 — heavy squash for very quiet rooms / laptop speakers.
    strong: "acompressor=threshold=0.032:ratio=8:attack=10:release=200:makeup=3",
};

export const NIGHT_MODE_LABELS: Record<NightModeLevel, string> = {
    off: "Off",
    light: "Light",
    medium: "Medium",
    strong: "Strong",
};

// `level=0` disables alimiter's auto-levelling, which would otherwise re-apply
// the gain we just limited away.
const LIMITER = "alimiter=limit=0.95:level=0";

const DEFAULT_STATE: AudioEnhancementsState = {
    nightMode: "off",
    dialogueBoost: 0,
    gain: 100,
    eqEnabled: false,
    eqBands: new Array(EQ_BAND_COUNT).fill(0),
    eqPreset: "flat",
};

const clamp = (value: number, min: number, max: number) =>
    Math.min(max, Math.max(min, value));

const isNightModeLevel = (value: unknown): value is NightModeLevel =>
    value === "off" ||
    value === "light" ||
    value === "medium" ||
    value === "strong";

const normalizeBands = (input: unknown): number[] => {
    const bands = new Array(EQ_BAND_COUNT).fill(0);
    if (!Array.isArray(input)) return bands;
    for (let i = 0; i < EQ_BAND_COUNT; i += 1) {
        const value = Number(input[i]);
        if (Number.isFinite(value)) {
            bands[i] = clamp(Math.round(value * 10) / 10, -EQ_GAIN_RANGE, EQ_GAIN_RANGE);
        }
    }
    return bands;
};

const bandsMatch = (a: number[], b: number[]) =>
    a.length === b.length && a.every((value, i) => Math.abs(value - b[i]) < 0.05);

/** One `equalizer` biquad per non-zero band, chained into a single graph. */
const buildEqGraph = (bands: number[]): string =>
    bands
        .map((gain, i) =>
            Math.abs(gain) < 0.05
                ? null
                : `equalizer=f=${EQ_FREQUENCIES[i]}:width_type=o:width=1:g=${gain.toFixed(1)}`,
        )
        .filter((part): part is string => part !== null)
        .join(",");

// Dialogue boost: lift the 2-4 kHz presence range where consonants live and
// scoop a little 300 Hz mud, which is what makes downmixed 5.1 centre channels
// sound buried. 0..100 maps to roughly 0..+6 dB of presence.
const buildDialogueGraph = (amount: number): string => {
    const strength = clamp(amount, 0, 100) / 100;
    const presence = (strength * 6).toFixed(1);
    const mud = (strength * -3).toFixed(1);
    return [
        `equalizer=f=300:width_type=o:width=1.2:g=${mud}`,
        `equalizer=f=2500:width_type=o:width=1.4:g=${presence}`,
        `equalizer=f=4000:width_type=o:width=1.4:g=${(strength * 3).toFixed(1)}`,
    ].join(",");
};

export const useAudioEnhancements = () => {
    const state = reactive<AudioEnhancementsState>({
        ...DEFAULT_STATE,
        eqBands: [...DEFAULT_STATE.eqBands],
    });
    const saver = createDebouncedUiStateSaver(300);

    let loaded = false;
    let onMessage: ((text: string) => void) | null = null;
    const setMessageHandler = (handler: (text: string) => void) => {
        onMessage = handler;
    };

    const runCommand = async (args: Array<string | number>) => {
        try {
            await invoke("mpv_run_command", { args });
        } catch (error) {
            // `af remove` on an absent label errors harmlessly — swallow it.
            console.warn("[audio] mpv command failed", { args, error });
        }
    };

    const persist = () => {
        saver.saveDebounced({
            audioEnhancements: {
                nightMode: state.nightMode,
                dialogueBoost: state.dialogueBoost,
                gain: state.gain,
                eqEnabled: state.eqEnabled,
                eqBands: [...state.eqBands],
                eqPreset: state.eqPreset,
            },
        });
    };

    // --- mpv application ----------------------------------------------------

    // Rebuild the whole labelled chain in a fixed order. Removing first keeps
    // this idempotent, so it doubles as "reset to the current state" after a
    // file load (mpv keeps af across files, but a failed load can drop it).
    let rebuilding: Promise<void> = Promise.resolve();
    const rebuildFilters = async () => {
        const run = async () => {
            for (const label of ALL_LABELS) {
                await runCommand(["af", "remove", `@${label}`]);
            }

            if (state.eqEnabled) {
                const graph = buildEqGraph(state.eqBands);
                if (graph) {
                    await runCommand(["af", "add", `@${LABEL_EQ}:lavfi=[${graph}]`]);
                }
            }

            if (state.dialogueBoost > 0) {
                const graph = buildDialogueGraph(state.dialogueBoost);
                await runCommand([
                    "af",
                    "add",
                    `@${LABEL_DIALOGUE}:lavfi=[${graph}]`,
                ]);
            }

            if (state.nightMode !== "off") {
                const graph = NIGHT_MODE_GRAPHS[state.nightMode];
                await runCommand([
                    "af",
                    "add",
                    `@${LABEL_NIGHT}:lavfi=[${graph},${LIMITER}]`,
                ]);
            }

            if (state.gain > 100) {
                const factor = (clamp(state.gain, 100, 300) / 100).toFixed(2);
                await runCommand([
                    "af",
                    "add",
                    `@${LABEL_GAIN}:lavfi=[volume=${factor},${LIMITER}]`,
                ]);
            }
        };
        // Serialise rebuilds so overlapping slider drags can't interleave
        // add/remove pairs and leave a stale filter attached.
        rebuilding = rebuilding.then(run, run);
        await rebuilding;
    };

    let rebuildTimer: number | null = null;
    const scheduleRebuild = () => {
        if (rebuildTimer) window.clearTimeout(rebuildTimer);
        rebuildTimer = window.setTimeout(() => {
            rebuildTimer = null;
            void rebuildFilters();
        }, 120);
    };

    // --- public setters -----------------------------------------------------

    const setNightMode = async (level: NightModeLevel) => {
        state.nightMode = level;
        await rebuildFilters();
        persist();
        onMessage?.(
            level === "off"
                ? "Night mode off"
                : `Night mode · ${NIGHT_MODE_LABELS[level]}`,
        );
    };

    const cycleNightMode = async () => {
        const order: NightModeLevel[] = ["off", "light", "medium", "strong"];
        const next = order[(order.indexOf(state.nightMode) + 1) % order.length];
        await setNightMode(next);
    };

    const setDialogueBoost = (value: number) => {
        state.dialogueBoost = clamp(Math.round(value), 0, 100);
        scheduleRebuild();
        persist();
    };

    const setGain = (value: number) => {
        state.gain = clamp(Math.round(value), 100, 300);
        scheduleRebuild();
        persist();
    };

    const setEqEnabled = async (enabled: boolean) => {
        state.eqEnabled = enabled;
        await rebuildFilters();
        persist();
        onMessage?.(`Equalizer ${enabled ? "on" : "off"}`);
    };

    const setEqBand = (index: number, gain: number) => {
        if (index < 0 || index >= EQ_BAND_COUNT) return;
        state.eqBands[index] = clamp(
            Math.round(gain * 10) / 10,
            -EQ_GAIN_RANGE,
            EQ_GAIN_RANGE,
        );
        // A hand-edit detaches the band set from its preset unless it happens
        // to still match one.
        const match = EQ_PRESETS.find((preset) =>
            bandsMatch(preset.bands, state.eqBands),
        );
        state.eqPreset = match?.id ?? "";
        if (state.eqEnabled) scheduleRebuild();
        persist();
    };

    const applyEqPreset = async (id: string) => {
        const preset = EQ_PRESETS.find((candidate) => candidate.id === id);
        if (!preset) return;
        state.eqBands = [...preset.bands];
        state.eqPreset = preset.id;
        // Choosing a preset implies you want to hear it.
        if (!state.eqEnabled && preset.id !== "flat") state.eqEnabled = true;
        await rebuildFilters();
        persist();
        onMessage?.(`Equalizer · ${preset.name}`);
    };

    const resetEq = async () => {
        state.eqBands = new Array(EQ_BAND_COUNT).fill(0);
        state.eqPreset = "flat";
        await rebuildFilters();
        persist();
    };

    const reset = async () => {
        state.nightMode = DEFAULT_STATE.nightMode;
        state.dialogueBoost = DEFAULT_STATE.dialogueBoost;
        state.gain = DEFAULT_STATE.gain;
        state.eqEnabled = DEFAULT_STATE.eqEnabled;
        state.eqBands = new Array(EQ_BAND_COUNT).fill(0);
        state.eqPreset = "flat";
        await rebuildFilters();
        persist();
        onMessage?.("Audio reset");
    };

    /** True when anything is altering the audio (drives the button's dot). */
    const isActive = computed(
        () =>
            state.nightMode !== "off" ||
            state.dialogueBoost > 0 ||
            state.gain > 100 ||
            (state.eqEnabled && state.eqBands.some((g) => Math.abs(g) >= 0.05)),
    );

    // --- lifecycle ----------------------------------------------------------

    const onFileLoaded = async () => {
        if (!loaded) return;
        await rebuildFilters();
    };

    const load = async (stored?: StoredAudioEnhancements) => {
        if (stored) {
            if (isNightModeLevel(stored.nightMode)) {
                state.nightMode = stored.nightMode;
            }
            if (typeof stored.dialogueBoost === "number") {
                state.dialogueBoost = clamp(Math.round(stored.dialogueBoost), 0, 100);
            }
            if (typeof stored.gain === "number") {
                state.gain = clamp(Math.round(stored.gain), 100, 300);
            }
            state.eqEnabled = stored.eqEnabled === true;
            state.eqBands = normalizeBands(stored.eqBands);
            state.eqPreset =
                typeof stored.eqPreset === "string" ? stored.eqPreset : "";
        }
        loaded = true;
        await rebuildFilters();
    };

    onMounted(async () => {
        const stored = await loadUiState<{
            audioEnhancements?: StoredAudioEnhancements;
        }>();
        await load(stored?.audioEnhancements);
    });

    return {
        state,
        isActive,
        setNightMode,
        cycleNightMode,
        setDialogueBoost,
        setGain,
        setEqEnabled,
        setEqBand,
        applyEqPreset,
        resetEq,
        reset,
        setMessageHandler,
        onFileLoaded,
    };
};

export type AudioEnhancementsController = ReturnType<typeof useAudioEnhancements>;

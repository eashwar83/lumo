import { computed, onMounted, ref, watch } from "vue";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";
import type { VideoEnhancementsController } from "./useVideoEnhancements";
import type { AudioEnhancementsController } from "./useAudioEnhancements";
import type {
    AiUpscaleMode,
    DebandLevel,
} from "./useVideoEnhancements";
import type { NightModeLevel } from "./useAudioEnhancements";

// Coordinates the two enhancement conveniences that span every picture/audio
// control:
//
//   * View Original — a bypass that pushes neutral values to mpv while leaving
//     your stored settings untouched. Toggle back to restore.
//   * Undo / redo   — a per-file history of enhancement snapshots. A settled
//     change (after a short debounce) pushes a snapshot; undo/redo walk it.
//
// The snapshot spans colour adjustments, the video look, and audio — everything
// you tune. Scaler quality is intentionally excluded (fidelity, not a look).

// The minimal slice of the colour-adjustment controller this needs.
type AdjustmentsLike = {
    brightness: { value: number };
    contrast: { value: number };
    saturation: { value: number };
    gamma: { value: number };
    hue: { value: number };
    setBrightness: (v: number) => Promise<void> | void;
    setContrast: (v: number) => Promise<void> | void;
    setSaturation: (v: number) => Promise<void> | void;
    setGamma: (v: number) => Promise<void> | void;
    setHue: (v: number) => Promise<void> | void;
    applyNeutral: () => Promise<void> | void;
    reapplyColorAdjustments: () => Promise<void> | void;
};

type Snapshot = {
    brightness: number;
    contrast: number;
    saturation: number;
    gamma: number;
    hue: number;
    sharpenAmount: number;
    sharpenRadius: number;
    denoise: boolean;
    deinterlace: boolean;
    exposure: number;
    temperature: number;
    tint: number;
    highlights: number;
    shadows: number;
    grain: number;
    curves: string;
    deband: DebandLevel;
    aiUpscale: AiUpscaleMode;
    nightMode: NightModeLevel;
    voiceIsolation: number;
    dialogueBoost: number;
    gain: number;
    eqEnabled: boolean;
    eqBands: number[];
    eqPreset: string;
};

// Each stack entry pairs the state with optional provenance: a short source
// label (e.g. the AI model), plus — for AI changes — the prompt used and the
// model's own description of what it did. All are metadata only; none take part
// in the equality check that dedupes identical states.
type StackEntry = {
    snap: Snapshot;
    label?: string;
    prompt?: string;
    notes?: string;
};
type FileHistory = { stack: StackEntry[]; index: number };

const MAX_STEPS = 20;
const MAX_FILES = 60;
/** A change is "settled" this long after the last keystroke/drag tick. */
const CHECKPOINT_DEBOUNCE_MS = 500;
/** Suppress checkpoints a touch longer than the debounce during restore/load. */
const SUPPRESS_MS = 650;

type UseEnhancementHistoryOptions = {
    enhancements: VideoEnhancementsController;
    audio: AudioEnhancementsController;
    adjustments: AdjustmentsLike;
    onMessage?: (text: string) => void;
    /** Re-set after a silent restore so later changes narrate normally. */
    restoreMessageHandlers: () => void;
    /** Silence the composables' own OSD toasts during a restore. */
    silenceMessageHandlers: () => void;
};

export const useEnhancementHistory = (options: UseEnhancementHistoryOptions) => {
    const { enhancements, audio, adjustments } = options;

    const bypass = ref(false);
    const histories = new Map<string, FileHistory>();
    const currentKey = ref("");
    // While true, state changes are the app's doing (restore / file load), not
    // the user's, so no checkpoint is recorded.
    let suppress = false;
    let suppressTimer: number | null = null;
    let checkpointTimer: number | null = null;
    const saver = createDebouncedUiStateSaver(600);
    const revision = ref(0); // bumped so canUndo/canRedo recompute

    // Provenance to attach to the NEXT recorded change (e.g. the AI model, the
    // prompt, and the model's notes). Cleared shortly after so it can't leak
    // onto an unrelated later edit.
    let pendingLabel: string | undefined;
    let pendingPrompt: string | undefined;
    let pendingNotes: string | undefined;
    let pendingLabelTimer: number | null = null;
    const tagNextChange = (meta: {
        label: string;
        prompt?: string;
        notes?: string;
    }) => {
        pendingLabel = meta.label;
        pendingPrompt = meta.prompt?.trim() || undefined;
        pendingNotes = meta.notes?.trim() || undefined;
        if (pendingLabelTimer) window.clearTimeout(pendingLabelTimer);
        pendingLabelTimer = window.setTimeout(() => {
            pendingLabel = undefined;
            pendingPrompt = undefined;
            pendingNotes = undefined;
            pendingLabelTimer = null;
        }, 4000);
    };

    const capture = (): Snapshot => {
        const s = enhancements.state;
        const a = audio.state;
        return {
            brightness: adjustments.brightness.value,
            contrast: adjustments.contrast.value,
            saturation: adjustments.saturation.value,
            gamma: adjustments.gamma.value,
            hue: adjustments.hue.value,
            sharpenAmount: s.sharpenAmount,
            sharpenRadius: s.sharpenRadius,
            denoise: s.denoise,
            deinterlace: s.deinterlace,
            exposure: s.exposure,
            temperature: s.temperature,
            tint: s.tint,
            highlights: s.highlights,
            shadows: s.shadows,
            grain: s.grain,
            curves: s.curves,
            deband: s.deband,
            aiUpscale: s.aiUpscale,
            nightMode: a.nightMode,
            voiceIsolation: a.voiceIsolation,
            dialogueBoost: a.dialogueBoost,
            gain: a.gain,
            eqEnabled: a.eqEnabled,
            eqBands: [...a.eqBands],
            eqPreset: a.eqPreset,
        };
    };

    const serialize = (snap: Snapshot) => JSON.stringify(snap);

    const holdSuppression = () => {
        suppress = true;
        if (suppressTimer) window.clearTimeout(suppressTimer);
        suppressTimer = window.setTimeout(() => {
            suppress = false;
            suppressTimer = null;
        }, SUPPRESS_MS);
    };

    // Push snapshots into mpv via the real setters (so they persist), but with
    // the composables' toasts silenced — restoring 25 values shouldn't spray 6
    // overlays.
    const applySnapshot = async (snap: Snapshot) => {
        holdSuppression();
        options.silenceMessageHandlers();
        try {
            await adjustments.setBrightness(snap.brightness);
            await adjustments.setContrast(snap.contrast);
            await adjustments.setSaturation(snap.saturation);
            await adjustments.setGamma(snap.gamma);
            await adjustments.setHue(snap.hue);

            enhancements.setSharpenAmount(snap.sharpenAmount);
            enhancements.setSharpenRadius(snap.sharpenRadius);
            await enhancements.setDenoise(snap.denoise);
            await enhancements.setDeinterlace(snap.deinterlace);
            enhancements.setColorGrade("exposure", snap.exposure);
            enhancements.setColorGrade("temperature", snap.temperature);
            enhancements.setColorGrade("tint", snap.tint);
            enhancements.setColorGrade("highlights", snap.highlights);
            enhancements.setColorGrade("shadows", snap.shadows);
            enhancements.setGrain(snap.grain);
            enhancements.setCurves(snap.curves);
            await enhancements.setDeband(snap.deband);
            await enhancements.setAiUpscale(snap.aiUpscale);

            await audio.setNightMode(snap.nightMode);
            audio.setVoiceIsolation(snap.voiceIsolation);
            audio.setDialogueBoost(snap.dialogueBoost);
            audio.setGain(snap.gain);
            audio.setEqBands(snap.eqBands);
            await audio.setEqEnabled(snap.eqEnabled);
        } finally {
            options.restoreMessageHandlers();
        }
    };

    // --- View Original bypass ----------------------------------------------

    // Only pushes values to mpv; never mutates stored state, so it can't
    // trigger a checkpoint and must NOT suppress one — otherwise a change made
    // while viewing the original (which auto-exits bypass) would go unrecorded,
    // leaving the undo pointer a step behind the actual state.
    const applyBypass = async (on: boolean) => {
        if (on) {
            await adjustments.applyNeutral();
            await enhancements.applyNeutral();
            await audio.applyNeutral();
        } else {
            await adjustments.reapplyColorAdjustments();
            await enhancements.reapplyAll();
            await audio.reapplyToMpv();
        }
    };

    const setBypass = async (on: boolean) => {
        if (bypass.value === on) return;
        bypass.value = on;
        await applyBypass(on);
        options.onMessage?.(on ? "Viewing original" : "Viewing enhanced");
    };

    const toggleBypass = () => void setBypass(!bypass.value);

    // --- undo / redo --------------------------------------------------------

    const checkpoint = () => {
        if (suppress) return;
        const snap = capture();
        const existing = histories.get(currentKey.value) ?? {
            stack: [],
            index: -1,
        };
        const top = existing.stack[existing.index];
        if (top && serialize(top.snap) === serialize(snap)) return;
        // Drop any redo branch, append, cap depth.
        const stack = existing.stack.slice(0, existing.index + 1);
        stack.push({
            snap,
            label: pendingLabel,
            prompt: pendingPrompt,
            notes: pendingNotes,
        });
        pendingLabel = undefined;
        pendingPrompt = undefined;
        pendingNotes = undefined;
        while (stack.length > MAX_STEPS) stack.shift();
        histories.set(currentKey.value, { stack, index: stack.length - 1 });
        trimFiles();
        revision.value += 1;
        persist();
    };

    const trimFiles = () => {
        while (histories.size > MAX_FILES) {
            const oldest = histories.keys().next().value;
            if (oldest === undefined || oldest === currentKey.value) break;
            histories.delete(oldest);
        }
    };

    /** Trim a caption to keep the on-screen readout from ballooning. */
    const clip = (text: string, max: number): string =>
        text.length > max ? `${text.slice(0, max - 1).trimEnd()}…` : text;

    // Position readout: numerator is the current step (0 = original), the
    // denominator is how many changes exist in this file's history branch. For
    // an AI change, the prompt and the model's notes are added on their own
    // lines (the message overlay renders newlines when present).
    const positionLabel = (h: FileHistory): string => {
        const total = h.stack.length - 1;
        const entry = h.stack[h.index];
        const base =
            h.index === 0
                ? `Original · 0/${total}`
                : `Change ${h.index}/${total}`;
        const lines = [entry?.label ? `${base} · ${entry.label}` : base];
        if (entry?.prompt) lines.push(`Prompt: ${clip(entry.prompt, 140)}`);
        if (entry?.notes) lines.push(`Result: ${clip(entry.notes, 160)}`);
        return lines.join("\n");
    };

    const undo = async () => {
        if (bypass.value) await setBypass(false);
        const h = histories.get(currentKey.value);
        if (!h || h.index <= 0) {
            options.onMessage?.("Nothing to undo");
            return;
        }
        h.index -= 1;
        await applySnapshot(h.stack[h.index].snap);
        revision.value += 1;
        persist();
        options.onMessage?.(positionLabel(h));
    };

    const redo = async () => {
        if (bypass.value) await setBypass(false);
        const h = histories.get(currentKey.value);
        if (!h || h.index >= h.stack.length - 1) {
            options.onMessage?.("Nothing to redo");
            return;
        }
        h.index += 1;
        await applySnapshot(h.stack[h.index].snap);
        revision.value += 1;
        persist();
        options.onMessage?.(positionLabel(h));
    };

    const canUndo = computed(() => {
        void revision.value;
        const h = histories.get(currentKey.value);
        return !!h && h.index > 0;
    });
    const canRedo = computed(() => {
        void revision.value;
        const h = histories.get(currentKey.value);
        return !!h && h.index < h.stack.length - 1;
    });

    // --- persistence --------------------------------------------------------

    const persist = () => {
        const object: Record<string, FileHistory> = {};
        histories.forEach((value, key) => {
            object[key] = { stack: value.stack, index: value.index };
        });
        saver.saveDebounced({ enhancementHistory: object });
    };

    // --- watching for settled changes --------------------------------------

    watch(
        () => serialize(capture()),
        () => {
            // A user edit while viewing the original should snap back to the
            // enhanced view so they see what they changed.
            if (bypass.value && !suppress) {
                void setBypass(false);
            }
            if (suppress) return;
            if (checkpointTimer) window.clearTimeout(checkpointTimer);
            checkpointTimer = window.setTimeout(() => {
                checkpointTimer = null;
                checkpoint();
            }, CHECKPOINT_DEBOUNCE_MS);
        },
    );

    const onFileLoaded = (key: string) => {
        const normalized = key.trim();
        currentKey.value = normalized;
        // The file's enhancements are still being applied; suppress checkpoints,
        // then seed a baseline once they've settled (unless we already have a
        // persisted history for this file).
        holdSuppression();
        window.setTimeout(() => {
            if (currentKey.value !== normalized) return;
            if (!histories.has(normalized)) {
                histories.set(normalized, {
                    stack: [{ snap: capture() }],
                    index: 0,
                });
                trimFiles();
            }
            revision.value += 1;
        }, 450);
    };

    onMounted(async () => {
        const stored = await loadUiState<{
            enhancementHistory?: Record<
                string,
                { stack: unknown[]; index: number }
            >;
        }>();
        const map = stored?.enhancementHistory;
        if (!map) return;
        Object.entries(map).forEach(([key, value]) => {
            const normalized = key.trim();
            if (!normalized || !Array.isArray(value?.stack)) return;
            // Accept both the new { snap, label } entries and the older bare
            // Snapshot entries (migrate the latter).
            const entries: StackEntry[] = value.stack.map((raw) => {
                const obj = raw as Record<string, unknown>;
                if (obj && typeof obj === "object" && "snap" in obj) {
                    return {
                        snap: obj.snap as Snapshot,
                        label:
                            typeof obj.label === "string" ? obj.label : undefined,
                        prompt:
                            typeof obj.prompt === "string" ? obj.prompt : undefined,
                        notes:
                            typeof obj.notes === "string" ? obj.notes : undefined,
                    };
                }
                return { snap: raw as Snapshot };
            });
            const stack = entries.slice(-MAX_STEPS);
            const index = Math.min(
                Math.max(0, value.index ?? stack.length - 1),
                stack.length - 1,
            );
            if (stack.length) histories.set(normalized, { stack, index });
        });
    });

    return {
        bypass,
        toggleBypass,
        setBypass,
        undo,
        redo,
        canUndo,
        canRedo,
        onFileLoaded,
        tagNextChange,
    };
};

export type EnhancementHistoryController = ReturnType<
    typeof useEnhancementHistory
>;

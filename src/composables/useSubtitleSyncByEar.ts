import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// Subtitle sync by ear.
//
// Nudging the delay ±0.1s and re-watching the same line is a miserable way to
// fix scraped subtitles. Instead: play the scene, and tap the key at the moment
// a line *should* appear. mpv already knows when the nearest subtitle event
// actually starts (`sub-start`, relative to the current delay), so the required
// correction is simply the difference — one keypress instead of twenty.

type UseSubtitleSyncByEarOptions = {
    getDelay: () => number;
    setDelay: (seconds: number) => void | Promise<void>;
    onMessage?: (text: string) => void;
};

/** Beyond this the tap almost certainly refers to a different line. */
const MAX_CORRECTION_SECONDS = 30;

const readNumber = async (name: string): Promise<number | null> => {
    try {
        const raw = await invoke<string | null>("mpv_get_property_string", { name });
        if (raw === null || raw === undefined) return null;
        const value = Number.parseFloat(String(raw).trim());
        return Number.isFinite(value) ? value : null;
    } catch {
        return null;
    }
};

export const useSubtitleSyncByEar = ({
    getDelay,
    setDelay,
    onMessage,
}: UseSubtitleSyncByEarOptions) => {
    const isBusy = ref(false);

    const syncNow = async () => {
        if (isBusy.value) return;
        isBusy.value = true;
        try {
            // `sub-start` is the displayed start of the current/last subtitle,
            // already including the active delay, in seconds from file start.
            const subStart = await readNumber("sub-start");
            const position = await readNumber("time-pos");
            if (subStart === null || position === null) {
                onMessage?.("No subtitle line to sync to");
                return;
            }

            // Positive when the line appeared too early: shift it later.
            const correction = position - subStart;
            if (Math.abs(correction) > MAX_CORRECTION_SECONDS) {
                onMessage?.("Too far from a subtitle line to sync");
                return;
            }
            if (Math.abs(correction) < 0.02) {
                onMessage?.("Subtitles already in sync");
                return;
            }

            const next = Math.round((getDelay() + correction) * 100) / 100;
            await setDelay(next);
            const sign = correction > 0 ? "+" : "−";
            onMessage?.(
                `Subtitles shifted ${sign}${Math.abs(correction).toFixed(2)}s · delay ${next.toFixed(2)}s`,
            );
        } finally {
            isBusy.value = false;
        }
    };

    return { isBusy, syncNow };
};

export type SubtitleSyncByEarController = ReturnType<typeof useSubtitleSyncByEar>;

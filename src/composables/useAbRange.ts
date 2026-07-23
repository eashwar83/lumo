import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// A-B range selection. One control marks the in point, then the out point, then
// clears — the same cycle VLC and mpv use. Once both ends are set the range
// loops via mpv's own `ab-loop-a`/`ab-loop-b` (frame-accurate, no polling), and
// it doubles as the source range for clip/GIF export and for saving a series'
// intro markers.

export type AbRange = { a: number | null; b: number | null };

/** Shortest range worth keeping; anything less is a mis-click. */
const MIN_RANGE_SECONDS = 0.25;

type UseAbRangeOptions = {
    getPosition: () => number;
    getDuration: () => number;
    isFileLoaded: () => boolean;
    onMessage?: (text: string) => void;
};

export const useAbRange = ({
    getPosition,
    getDuration,
    isFileLoaded,
    onMessage,
}: UseAbRangeOptions) => {
    const pointA = ref<number | null>(null);
    const pointB = ref<number | null>(null);
    const loopEnabled = ref(true);

    const hasRange = computed(
        () => pointA.value !== null && pointB.value !== null,
    );
    const isActive = computed(() => pointA.value !== null);
    const rangeLength = computed(() =>
        hasRange.value ? Math.max(0, pointB.value! - pointA.value!) : 0,
    );

    const setProp = async (name: string, value: string) => {
        try {
            await invoke("mpv_set_option_string", { name, value });
        } catch (error) {
            console.warn("[ab-range] set option failed", { name, value, error });
        }
    };

    // mpv treats "no" as "unset" for the ab-loop points. The loop only engages
    // when both ends are pushed, so a half-set range just sits there as a marker.
    const syncLoop = async () => {
        const wantLoop = loopEnabled.value && hasRange.value;
        await setProp("ab-loop-a", wantLoop ? String(pointA.value) : "no");
        await setProp("ab-loop-b", wantLoop ? String(pointB.value) : "no");
    };

    const clear = async (silent = false) => {
        pointA.value = null;
        pointB.value = null;
        await syncLoop();
        if (!silent) onMessage?.("A-B range cleared");
    };

    const markA = async (at?: number) => {
        if (!isFileLoaded()) return;
        const position = at ?? getPosition();
        pointA.value = Math.max(0, position);
        // Marking a new A above an existing B would invert the range.
        if (pointB.value !== null && pointB.value <= pointA.value) {
            pointB.value = null;
        }
        await syncLoop();
        onMessage?.("A-B: start marked");
    };

    const markB = async (at?: number) => {
        if (!isFileLoaded() || pointA.value === null) return;
        const duration = getDuration();
        const position = Math.min(at ?? getPosition(), duration || Infinity);
        if (position - pointA.value < MIN_RANGE_SECONDS) {
            onMessage?.("A-B: range too short");
            return;
        }
        pointB.value = position;
        await syncLoop();
        onMessage?.(loopEnabled.value ? "A-B: looping range" : "A-B: end marked");
    };

    /** One control, three states: mark A -> mark B -> clear. */
    const cycle = async () => {
        if (!isFileLoaded()) return;
        if (pointA.value === null) {
            await markA();
            return;
        }
        if (pointB.value === null) {
            await markB();
            return;
        }
        await clear();
    };

    const setLoopEnabled = async (enabled: boolean) => {
        loopEnabled.value = enabled;
        await syncLoop();
        onMessage?.(enabled ? "A-B loop on" : "A-B loop off");
    };

    /** Drop the range when the file changes — the timestamps mean nothing there. */
    const onFileLoaded = async () => {
        await clear(true);
    };

    return {
        pointA,
        pointB,
        loopEnabled,
        hasRange,
        isActive,
        rangeLength,
        markA,
        markB,
        cycle,
        clear,
        setLoopEnabled,
        onFileLoaded,
    };
};

export type AbRangeController = ReturnType<typeof useAbRange>;

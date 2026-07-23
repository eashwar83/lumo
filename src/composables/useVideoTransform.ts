import { computed, onMounted, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";

// Zoom / pan / rotate, driven by mpv's `video-zoom`, `video-pan-x/y` and
// `video-rotate`. Remembered per file in the `perFileTransform` ui-state slice
// alongside (but separate from) the aspect/crop memory in useVideoGeometry.
//
// mpv's zoom is logarithmic: `video-zoom` 0 = 1x, 1 = 2x, -1 = 0.5x. Pan is
// expressed as a fraction of the *window* size, so the meaningful pan range
// grows with the zoom factor — see `maxPan`.

export type VideoTransform = {
    /** log2 zoom, 0 = fit. */
    zoom: number;
    panX: number;
    panY: number;
    /** Degrees, one of 0/90/180/270. */
    rotate: number;
};

type StoredPerFileTransform = Record<string, Partial<VideoTransform>>;

const MIN_ZOOM = -1; // 0.5x
const MAX_ZOOM = 3; // 8x
const ZOOM_STEP = 0.125; // ~9% per wheel notch
const KEY_ZOOM_STEP = 0.25;
const MAX_ENTRIES = 300;

const DEFAULT_TRANSFORM: VideoTransform = {
    zoom: 0,
    panX: 0,
    panY: 0,
    rotate: 0,
};

const clamp = (value: number, min: number, max: number) =>
    Math.min(max, Math.max(min, value));

const round3 = (value: number) => Math.round(value * 1000) / 1000;

/** Linear zoom factor from mpv's log2 `video-zoom`. */
const zoomFactor = (zoom: number) => Math.pow(2, zoom);

// How far the image can be panned before its edge crosses the window edge.
// At factor Z the overhang is (Z - 1) window-widths, half on each side.
const maxPan = (zoom: number) => Math.max(0, (zoomFactor(zoom) - 1) / 2);

const isDefaultTransform = (t: VideoTransform) =>
    Math.abs(t.zoom) < 0.001 &&
    Math.abs(t.panX) < 0.001 &&
    Math.abs(t.panY) < 0.001 &&
    t.rotate === 0;

export const useVideoTransform = () => {
    const state = reactive<VideoTransform>({ ...DEFAULT_TRANSFORM });
    const entries = new Map<string, VideoTransform>();
    const saver = createDebouncedUiStateSaver(400);

    let currentKey = "";
    let onMessage: ((text: string) => void) | null = null;
    const setMessageHandler = (handler: (text: string) => void) => {
        onMessage = handler;
    };

    const setProp = async (name: string, value: string | number) => {
        try {
            await invoke("mpv_set_option_string", { name, value: String(value) });
        } catch (error) {
            console.warn("[transform] set option failed", { name, value, error });
        }
    };

    const persist = () => {
        const object: StoredPerFileTransform = {};
        entries.forEach((value, key) => {
            object[key] = { ...value };
        });
        saver.saveDebounced({ perFileTransform: object });
    };

    // Re-insert to keep insertion order (newest last) and evict the oldest.
    const remember = () => {
        if (!currentKey) return;
        entries.delete(currentKey);
        // Nothing worth storing for a video at its default transform.
        if (!isDefaultTransform(state)) {
            entries.set(currentKey, { ...state });
            while (entries.size > MAX_ENTRIES) {
                const oldest = entries.keys().next().value;
                if (oldest === undefined) break;
                entries.delete(oldest);
            }
        }
        persist();
    };

    // Clamp the pan back inside range — needed after any zoom change, otherwise
    // zooming out would leave the image stranded off-centre.
    const clampPan = () => {
        const limit = maxPan(state.zoom);
        state.panX = clamp(state.panX, -limit, limit);
        state.panY = clamp(state.panY, -limit, limit);
    };

    const apply = async () => {
        await setProp("video-zoom", round3(state.zoom));
        await setProp("video-pan-x", round3(state.panX));
        await setProp("video-pan-y", round3(state.panY));
        await setProp("video-rotate", state.rotate);
    };

    // Coalesce the rapid property writes a wheel-zoom or a pan drag produces.
    let applyTimer: number | null = null;
    const scheduleApply = () => {
        if (applyTimer) window.clearTimeout(applyTimer);
        applyTimer = window.setTimeout(() => {
            applyTimer = null;
            void apply();
        }, 16);
    };

    const zoomPercent = computed(() => Math.round(zoomFactor(state.zoom) * 100));
    const isZoomed = computed(() => state.zoom > 0.001);
    const isTransformed = computed(() => !isDefaultTransform(state));

    /**
     * Zoom by `delta` (in log2 units), keeping the point under the cursor fixed.
     * `anchorX`/`anchorY` are the cursor position as a 0..1 fraction of the
     * video area; omit them to zoom about the centre.
     */
    const zoomBy = (delta: number, anchorX = 0.5, anchorY = 0.5) => {
        const previous = state.zoom;
        const next = clamp(previous + delta, MIN_ZOOM, MAX_ZOOM);
        if (Math.abs(next - previous) < 0.0001) return;

        // Solve pan so the source point under the cursor doesn't move:
        //   u - pan1 = (Z1/Z0) * (u - pan0)
        // with u the cursor offset from centre in window-size fractions.
        const ratio = zoomFactor(next) / zoomFactor(previous);
        const offsetX = clamp(anchorX, 0, 1) - 0.5;
        const offsetY = clamp(anchorY, 0, 1) - 0.5;
        state.panX = offsetX - ratio * (offsetX - state.panX);
        state.panY = offsetY - ratio * (offsetY - state.panY);
        state.zoom = next;
        clampPan();
        scheduleApply();
        remember();
    };

    const zoomIn = () => {
        zoomBy(KEY_ZOOM_STEP);
        onMessage?.(`Zoom ${zoomPercent.value}%`);
    };

    const zoomOut = () => {
        zoomBy(-KEY_ZOOM_STEP);
        onMessage?.(`Zoom ${zoomPercent.value}%`);
    };

    /** Pan by a fraction of the window size (positive dx moves the image right). */
    const panBy = (dx: number, dy: number) => {
        if (!isZoomed.value) return;
        const limit = maxPan(state.zoom);
        state.panX = clamp(state.panX + dx, -limit, limit);
        state.panY = clamp(state.panY + dy, -limit, limit);
        scheduleApply();
        remember();
    };

    const rotateBy = async (degrees: number) => {
        state.rotate = (((state.rotate + degrees) % 360) + 360) % 360;
        await apply();
        remember();
        onMessage?.(
            state.rotate === 0 ? "Rotation reset" : `Rotated ${state.rotate}°`,
        );
    };

    const reset = async () => {
        Object.assign(state, DEFAULT_TRANSFORM);
        await apply();
        remember();
        onMessage?.("Zoom & rotation reset");
    };

    /** Reset zoom/pan but keep the rotation — used by the "fit" affordances. */
    const resetZoom = async () => {
        state.zoom = 0;
        state.panX = 0;
        state.panY = 0;
        await apply();
        remember();
    };

    // Always apply explicitly on load so a previous file's zoom can't leak into
    // a file with no saved transform.
    const applyForMedia = async (key: string) => {
        currentKey = key.trim();
        const saved = entries.get(currentKey);
        Object.assign(state, DEFAULT_TRANSFORM, saved ?? {});
        clampPan();
        await apply();
    };

    const load = (stored?: StoredPerFileTransform) => {
        if (!stored) return;
        Object.entries(stored).forEach(([key, value]) => {
            const normalizedKey = key.trim();
            if (!normalizedKey) return;
            entries.set(normalizedKey, {
                zoom: clamp(Number(value.zoom) || 0, MIN_ZOOM, MAX_ZOOM),
                panX: Number(value.panX) || 0,
                panY: Number(value.panY) || 0,
                rotate: [0, 90, 180, 270].includes(Number(value.rotate))
                    ? Number(value.rotate)
                    : 0,
            });
        });
    };

    onMounted(async () => {
        const uiState = await loadUiState<{
            perFileTransform?: StoredPerFileTransform;
        }>();
        load(uiState?.perFileTransform);
    });

    return {
        state,
        zoomPercent,
        isZoomed,
        isTransformed,
        zoomBy,
        zoomIn,
        zoomOut,
        panBy,
        rotateBy,
        reset,
        resetZoom,
        applyForMedia,
        setMessageHandler,
        ZOOM_STEP,
    };
};

export type VideoTransformController = ReturnType<typeof useVideoTransform>;

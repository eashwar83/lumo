<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
    autoCurvesFromHistogram,
    type ChannelHistograms,
    cloneCurves,
    type CurveChannel,
    type CurvePoint,
    type Curves,
    defaultCurves,
    identityPoints,
    makeCurveEvaluator,
    parseCurves,
    serializeCurves,
} from "../utils/curves";
import {
    BUILT_IN_CURVE_PRESETS,
    blendCurvesTowardIdentity,
    CURVE_PRESET_CATEGORY_ORDER,
    type CurvePreset,
} from "../utils/curvePresets";
import { useCurvePresets } from "../composables/useCurvePresets";
import type { VideoEnhancementsController } from "../composables/useVideoEnhancements";

const props = defineProps<{
    enhancements: VideoEnhancementsController;
    visible: boolean;
    mediaPath?: string;
    duration?: number;
}>();

const emit = defineEmits<{ (e: "close"): void }>();

const SIZE = 256; // svg coordinate space (also the curve resolution)

const channel = ref<CurveChannel>("rgb");
const curves = reactive<Curves>(defaultCurves());
type Histogram = { r: number[]; g: number[]; b: number[]; luma: number[] };
const histogram = ref<Histogram | null>(null);

const svgRef = ref<SVGSVGElement | null>(null);
let dragIndex: number | null = null;

const CHANNELS: { id: CurveChannel; label: string; color: string }[] = [
    { id: "rgb", label: "RGB", color: "#e6e6e6" },
    { id: "r", label: "Red", color: "#ff5b5b" },
    { id: "g", label: "Green", color: "#4ad06a" },
    { id: "b", label: "Blue", color: "#5b8cff" },
];

const activeColor = computed(
    () => CHANNELS.find((c) => c.id === channel.value)?.color ?? "#e6e6e6",
);

const points = computed(() => curves[channel.value]);

// --- coordinate helpers ----------------------------------------------------
// Curve space: x,y in 0..1. Screen: y is flipped (1 = top).
const toScreenX = (x: number) => x * SIZE;
const toScreenY = (y: number) => (1 - y) * SIZE;

const eventToCurve = (event: PointerEvent): CurvePoint => {
    const rect = svgRef.value?.getBoundingClientRect();
    if (!rect || rect.width === 0 || rect.height === 0) return { x: 0, y: 0 };
    const x = (event.clientX - rect.left) / rect.width;
    const y = 1 - (event.clientY - rect.top) / rect.height;
    return {
        x: Math.min(1, Math.max(0, x)),
        y: Math.min(1, Math.max(0, y)),
    };
};

// --- curve line path (sampled through the spline) --------------------------
const curvePath = computed(() => {
    const evalCurve = makeCurveEvaluator(points.value);
    let d = "";
    const steps = 64;
    for (let i = 0; i <= steps; i++) {
        const x = i / steps;
        const y = evalCurve(x);
        d += `${i === 0 ? "M" : "L"}${toScreenX(x).toFixed(2)} ${toScreenY(y).toFixed(2)} `;
    }
    return d;
});

// --- histogram path (filled area for the active channel) -------------------
const histogramPath = computed(() => {
    const h = histogram.value;
    if (!h) return "";
    const bins =
        channel.value === "r"
            ? h.r
            : channel.value === "g"
              ? h.g
              : channel.value === "b"
                ? h.b
                : h.luma;
    if (!bins || bins.length === 0) return "";
    const n = bins.length;
    let d = `M0 ${SIZE} `;
    for (let i = 0; i < n; i++) {
        const x = (i / (n - 1)) * SIZE;
        const y = SIZE - Math.min(1, Math.max(0, bins[i])) * SIZE;
        d += `L${x.toFixed(2)} ${y.toFixed(2)} `;
    }
    d += `L${SIZE} ${SIZE} Z`;
    return d;
});

// --- editing ---------------------------------------------------------------
const emitChange = () => {
    props.enhancements.setCurves(serializeCurves(curves));
};

const nearestPointIndex = (p: CurvePoint): number | null => {
    const pts = points.value;
    const threshold = 0.045; // ~12px on a 256 box
    let best = -1;
    let bestDist = threshold;
    for (let i = 0; i < pts.length; i++) {
        const dist = Math.hypot(pts[i].x - p.x, pts[i].y - p.y);
        if (dist < bestDist) {
            bestDist = dist;
            best = i;
        }
    }
    return best >= 0 ? best : null;
};

const onPointerDown = (event: PointerEvent) => {
    event.preventDefault();
    clearPresetLink();
    const p = eventToCurve(event);
    const pts = points.value;
    const hit = nearestPointIndex(p);
    if (hit !== null) {
        dragIndex = hit;
    } else {
        // Insert a new control point at this x, keep sorted.
        let insertAt = pts.length;
        for (let i = 0; i < pts.length; i++) {
            if (p.x < pts[i].x) {
                insertAt = i;
                break;
            }
        }
        pts.splice(insertAt, 0, { x: p.x, y: p.y });
        dragIndex = insertAt;
        emitChange();
    }
    svgRef.value?.setPointerCapture(event.pointerId);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
};

const onPointerMove = (event: PointerEvent) => {
    if (dragIndex === null) return;
    const pts = points.value;
    const i = dragIndex;
    const p = eventToCurve(event);
    const isFirst = i === 0;
    const isLast = i === pts.length - 1;
    // Endpoints keep their x (0 / 1); interior points stay between neighbours.
    let x = p.x;
    if (isFirst) x = 0;
    else if (isLast) x = 1;
    else {
        const lo = pts[i - 1].x + 0.001;
        const hi = pts[i + 1].x - 0.001;
        x = Math.min(hi, Math.max(lo, p.x));
    }
    pts[i] = { x, y: p.y };
    emitChange();
};

const onPointerUp = () => {
    dragIndex = null;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
};

// Double-click a control point to delete it (endpoints can't be removed).
const onPointDblClick = (index: number) => {
    const pts = points.value;
    if (index <= 0 || index >= pts.length - 1) return;
    clearPresetLink();
    pts.splice(index, 1);
    emitChange();
};

const resetChannel = () => {
    clearPresetLink();
    curves[channel.value] = identityPoints();
    emitChange();
};

const resetAll = () => {
    clearPresetLink();
    const fresh = defaultCurves();
    curves.rgb = fresh.rgb;
    curves.r = fresh.r;
    curves.g = fresh.g;
    curves.b = fresh.b;
    emitChange();
};

const autoBusy = ref(false);
const autoError = ref("");
// Sample ~20 frames across the video, derive auto-levels curves, apply them.
const onAuto = async () => {
    if (autoBusy.value) return;
    const path = props.mediaPath;
    const duration = props.duration ?? 0;
    if (!path || duration <= 0) return;
    autoBusy.value = true;
    autoError.value = "";
    try {
        const hist = await invoke<ChannelHistograms>("analyze_video_curves", {
            path,
            duration,
        });
        const auto = autoCurvesFromHistogram(hist);
        clearPresetLink();
        curves.rgb = auto.rgb;
        curves.r = auto.r;
        curves.g = auto.g;
        curves.b = auto.b;
        emitChange();
    } catch (error) {
        autoError.value = "Auto failed";
        console.warn("[curves] auto failed", error);
    } finally {
        autoBusy.value = false;
    }
};

// --- Presets ---------------------------------------------------------------

const curvePresets = useCurvePresets();

// When a preset is applied we keep its full-strength curves as the "base" so the
// Strength slider can re-derive the displayed curves from it (never compounding)
// by blending toward identity. Hand-editing a point clears the link (below), so
// the slider only shows while a preset is genuinely active.
const activePresetBase = ref<Curves | null>(null);
const activePresetName = ref("");
const strength = ref(100);

const presetGroups = computed(() =>
    CURVE_PRESET_CATEGORY_ORDER.map((category) => ({
        category,
        items: BUILT_IN_CURVE_PRESETS.filter((p) => p.category === category),
    })).filter((group) => group.items.length > 0),
);

const setCurvesFrom = (next: Curves) => {
    curves.rgb = next.rgb.map((p) => ({ ...p }));
    curves.r = next.r.map((p) => ({ ...p }));
    curves.g = next.g.map((p) => ({ ...p }));
    curves.b = next.b.map((p) => ({ ...p }));
};

const applyStrength = () => {
    if (!activePresetBase.value) return;
    setCurvesFrom(
        blendCurvesTowardIdentity(activePresetBase.value, strength.value / 100),
    );
    emitChange();
};

const applyPresetCurves = (name: string, source: Curves) => {
    activePresetBase.value = cloneCurves(source);
    activePresetName.value = name;
    strength.value = 100;
    setCurvesFrom(source);
    emitChange();
};

const applyPreset = (item: CurvePreset) => {
    applyPresetCurves(item.name, item.curves);
};

// Hand-editing (dragging/adding/removing points, reset, auto) means the user has
// taken over — drop the preset link so the Strength slider disappears.
const clearPresetLink = () => {
    activePresetBase.value = null;
    activePresetName.value = "";
};

watch(strength, () => applyStrength());

// --- Save / remove custom presets ---
const presetSaving = ref(false);
const presetName = ref("");

const onConfirmSavePreset = () => {
    const name = presetName.value.trim();
    if (!name) return;
    curvePresets.saveCurrent(name, curves);
    presetName.value = "";
    presetSaving.value = false;
};
const onCancelSavePreset = () => {
    presetName.value = "";
    presetSaving.value = false;
};

const loadFromState = () => {
    const parsed = parseCurves(props.enhancements.state.curves);
    curves.rgb = parsed.rgb;
    curves.r = parsed.r;
    curves.g = parsed.g;
    curves.b = parsed.b;
};

const fetchHistogram = async () => {
    try {
        histogram.value = await invoke<Histogram>("capture_frame_histogram");
    } catch {
        histogram.value = null;
    }
};

// Reload curves + histogram each time the panel opens.
watch(
    () => props.visible,
    (open) => {
        if (open) {
            clearPresetLink();
            strength.value = 100;
            presetSaving.value = false;
            presetName.value = "";
            loadFromState();
            void fetchHistogram();
        }
    },
    { immediate: true },
);
</script>

<template>
    <transition name="curves-slide">
        <div v-if="visible" class="curves" data-window-no-drag>
            <div class="curves__header">
                <span class="curves__title">Curves</span>
                <button
                    class="curves__close"
                    type="button"
                    aria-label="Close curves"
                    @click="emit('close')"
                >
                    ✕
                </button>
            </div>

            <div class="curves__channels">
                <button
                    v-for="ch in CHANNELS"
                    :key="ch.id"
                    class="curves__chan"
                    :class="{ 'curves__chan--active': channel === ch.id }"
                    type="button"
                    :style="{ '--chan': ch.color }"
                    @click="channel = ch.id"
                >
                    {{ ch.label }}
                </button>
            </div>

            <button
                class="curves__auto"
                type="button"
                :disabled="autoBusy"
                @click="onAuto"
            >
                <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                    <path d="M7.5 5.6 5 7l1.4-2.5L5 2l2.5 1.4L10 2 8.6 4.5 10 7zM19 9l-1.3 2.9L15 13l2.7 1.1L19 17l1.3-2.9L23 13l-2.7-1.1zM11.5 11.5 9 6 6.5 11.5 1 14l5.5 2.5L9 22l2.5-5.5L17 14z" />
                </svg>
                <span>{{ autoBusy ? "Analyzing 20 frames…" : "Auto" }}</span>
            </button>
            <div v-if="autoError" class="curves__auto-error">{{ autoError }}</div>

            <div class="curves__editor">
                <svg
                    ref="svgRef"
                    class="curves__svg"
                    :viewBox="`0 0 ${SIZE} ${SIZE}`"
                    @pointerdown="onPointerDown"
                >
                    <path
                        v-if="histogramPath"
                        :d="histogramPath"
                        class="curves__hist"
                        :style="{ fill: activeColor }"
                    />
                    <g class="curves__grid">
                        <line
                            v-for="i in 3"
                            :key="`v${i}`"
                            :x1="(i * SIZE) / 4"
                            y1="0"
                            :x2="(i * SIZE) / 4"
                            :y2="SIZE"
                        />
                        <line
                            v-for="i in 3"
                            :key="`h${i}`"
                            x1="0"
                            :y1="(i * SIZE) / 4"
                            :x2="SIZE"
                            :y2="(i * SIZE) / 4"
                        />
                    </g>
                    <line
                        class="curves__identity"
                        x1="0"
                        :y1="SIZE"
                        :x2="SIZE"
                        y2="0"
                    />
                    <path
                        :d="curvePath"
                        class="curves__line"
                        :style="{ stroke: activeColor }"
                    />
                    <circle
                        v-for="(pt, i) in points"
                        :key="i"
                        :cx="toScreenX(pt.x)"
                        :cy="toScreenY(pt.y)"
                        r="5"
                        class="curves__point"
                        :style="{ stroke: activeColor }"
                        @pointerdown.stop="onPointerDown"
                        @dblclick.stop="onPointDblClick(i)"
                    />
                </svg>
            </div>

            <div class="curves__hint">
                Click to add a point · drag to shape · double-click to remove
            </div>

            <div class="curves__actions">
                <button class="curves__btn" type="button" @click="resetChannel">
                    Reset {{ CHANNELS.find((c) => c.id === channel)?.label }}
                </button>
                <button class="curves__btn" type="button" @click="resetAll">
                    Reset All
                </button>
            </div>

            <div v-if="activePresetBase" class="curves__strength">
                <div class="curves__strength-head">
                    <span class="curves__strength-label">
                        Strength<template v-if="activePresetName">
                            · {{ activePresetName }}</template
                        >
                    </span>
                    <span class="curves__strength-value">{{ strength }}%</span>
                </div>
                <input
                    v-model.number="strength"
                    class="curves__strength-input"
                    type="range"
                    min="0"
                    max="100"
                    step="1"
                    :style="{ '--v': `${strength}%` }"
                />
            </div>

            <div class="curves__presets">
                <div class="curves__presets-head">
                    <span class="curves__presets-title">Presets</span>
                    <button
                        v-if="!presetSaving"
                        class="curves__presets-save"
                        type="button"
                        @click="presetSaving = true"
                    >
                        + Save
                    </button>
                </div>

                <div v-if="presetSaving" class="curves__presets-saverow">
                    <input
                        v-model="presetName"
                        class="curves__presets-input"
                        type="text"
                        placeholder="Preset name"
                        maxlength="24"
                        @keydown.enter.stop="onConfirmSavePreset"
                        @keydown.stop
                    />
                    <button
                        class="curves__presets-btn curves__presets-btn--ok"
                        type="button"
                        @click="onConfirmSavePreset"
                    >
                        Save
                    </button>
                    <button
                        class="curves__presets-btn"
                        type="button"
                        @click="onCancelSavePreset"
                    >
                        Cancel
                    </button>
                </div>

                <div
                    v-for="group in presetGroups"
                    :key="group.category"
                    class="curves__presets-group"
                >
                    <div class="curves__presets-cat">{{ group.category }}</div>
                    <div class="curves__presets-chips">
                        <button
                            v-for="item in group.items"
                            :key="item.id"
                            class="curves__chip"
                            type="button"
                            @click="applyPreset(item)"
                        >
                            {{ item.name }}
                        </button>
                    </div>
                </div>

                <div
                    v-if="curvePresets.customPresets.value.length"
                    class="curves__presets-group"
                >
                    <div class="curves__presets-cat">My Presets</div>
                    <div class="curves__presets-chips">
                        <div
                            v-for="item in curvePresets.customPresets.value"
                            :key="item.id"
                            class="curves__chip curves__chip--custom"
                            role="button"
                            tabindex="0"
                            @click="applyPresetCurves(item.name, item.curves)"
                            @keydown.enter="
                                applyPresetCurves(item.name, item.curves)
                            "
                        >
                            <span class="curves__chip-name">{{ item.name }}</span>
                            <button
                                class="curves__chip-del"
                                type="button"
                                aria-label="Delete preset"
                                @click.stop="curvePresets.remove(item.id)"
                            >
                                ✕
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.curves {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 320px;
    max-width: 88vw;
    z-index: 115;
    display: flex;
    flex-direction: column;
    gap: 14px;
    /* Extra top padding clears the window title-bar controls (z-index 120). */
    padding: 46px 18px 24px;
    overflow-y: auto;
    background: rgba(18, 20, 24, 0.9);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border-left: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: -8px 0 30px rgba(0, 0, 0, 0.4);
    color: #fff;
    user-select: none;
    -webkit-user-select: none;
}

.curves__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.curves__title {
    font-size: 16px;
    font-weight: 700;
}

.curves__close {
    border: none;
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    cursor: pointer;
    font-size: 13px;
}

.curves__close:hover {
    background: rgba(255, 255, 255, 0.2);
}

.curves__channels {
    display: flex;
    gap: 6px;
}

.curves__chan {
    flex: 1;
    border: 1px solid rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.85);
    border-radius: 8px;
    padding: 7px 0;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s ease, border-color 0.15s ease;
}

.curves__chan--active {
    background: color-mix(in srgb, var(--chan) 26%, transparent);
    border-color: var(--chan);
    color: #fff;
}

.curves__auto {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    padding: 9px 12px;
    border: 1px solid rgba(143, 179, 255, 0.4);
    border-radius: 9px;
    background: rgba(143, 179, 255, 0.16);
    color: #dbe6ff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s ease;
}

.curves__auto:hover:not(:disabled) {
    background: rgba(143, 179, 255, 0.3);
}

.curves__auto:disabled {
    opacity: 0.65;
    cursor: default;
}

.curves__auto svg {
    width: 16px;
    height: 16px;
    color: #8fb3ff;
}

.curves__auto-error {
    font-size: 11.5px;
    color: #ff8a8a;
    text-align: center;
}

.curves__editor {
    /* Never let the flex column collapse the square editor when the window is
       short — keep its size and let the panel scroll instead. */
    flex: 0 0 auto;
    aspect-ratio: 1 / 1;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 10px;
    overflow: hidden;
}

.curves__svg {
    width: 100%;
    height: 100%;
    display: block;
    touch-action: none;
    cursor: crosshair;
}

.curves__hist {
    opacity: 0.28;
}

.curves__grid line {
    stroke: rgba(255, 255, 255, 0.1);
    stroke-width: 1;
}

.curves__identity {
    stroke: rgba(255, 255, 255, 0.25);
    stroke-width: 1;
    stroke-dasharray: 4 4;
}

.curves__line {
    fill: none;
    stroke-width: 2;
}

.curves__point {
    fill: #12141a;
    stroke-width: 2.5;
    cursor: grab;
}

.curves__point:hover {
    fill: #fff;
}

.curves__hint {
    font-size: 11.5px;
    color: rgba(255, 255, 255, 0.5);
    text-align: center;
    line-height: 1.4;
}

.curves__actions {
    display: flex;
    gap: 8px;
    margin-top: auto;
}

.curves__btn {
    flex: 1;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    border-radius: 8px;
    padding: 9px 0;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
}

.curves__btn:hover {
    background: rgba(255, 255, 255, 0.16);
}

.curves-slide-enter-active,
.curves-slide-leave-active {
    transition: transform 0.22s ease, opacity 0.22s ease;
}

.curves-slide-enter-from,
.curves-slide-leave-to {
    transform: translateX(20px);
    opacity: 0;
}

/* --- Strength slider --- */
.curves__strength {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 10px 12px;
    border: 1px solid rgba(143, 179, 255, 0.32);
    border-radius: 9px;
    background: rgba(143, 179, 255, 0.1);
}

.curves__strength-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
}

.curves__strength-label {
    color: #dbe6ff;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.curves__strength-value {
    color: rgba(219, 230, 255, 0.75);
    font-variant-numeric: tabular-nums;
    flex: 0 0 auto;
    padding-left: 8px;
}

.curves__strength-input {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(
        to right,
        #8fb3ff var(--v, 100%),
        rgba(255, 255, 255, 0.18) var(--v, 100%)
    );
    cursor: pointer;
}

.curves__strength-input::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #dbe6ff;
    border: 1px solid rgba(0, 0, 0, 0.2);
    cursor: pointer;
}

/* --- Presets --- */
.curves__presets {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-top: 4px;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
}

.curves__presets-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.curves__presets-title {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.55);
}

.curves__presets-save {
    border: none;
    background: transparent;
    color: #8fb3ff;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    padding: 2px 4px;
}

.curves__presets-save:hover {
    color: #b7ccff;
}

.curves__presets-saverow {
    display: flex;
    gap: 6px;
}

.curves__presets-input {
    flex: 1 1 auto;
    min-width: 0;
    padding: 6px 9px;
    border-radius: 7px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 12.5px;
}

.curves__presets-input:focus {
    outline: none;
    border-color: rgba(143, 179, 255, 0.6);
}

.curves__presets-btn {
    flex: 0 0 auto;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    border-radius: 7px;
    padding: 6px 10px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}

.curves__presets-btn:hover {
    background: rgba(255, 255, 255, 0.16);
}

.curves__presets-btn--ok {
    border-color: rgba(143, 179, 255, 0.5);
    background: rgba(143, 179, 255, 0.2);
    color: #dbe6ff;
}

.curves__presets-group {
    display: flex;
    flex-direction: column;
    gap: 7px;
}

.curves__presets-cat {
    font-size: 10.5px;
    font-weight: 650;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.42);
}

.curves__presets-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
}

.curves__chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(255, 255, 255, 0.07);
    color: rgba(255, 255, 255, 0.92);
    border-radius: 999px;
    padding: 6px 12px;
    font-size: 12.5px;
    font-weight: 550;
    cursor: pointer;
    transition: background-color 0.15s ease, border-color 0.15s ease;
}

.curves__chip:hover {
    background: rgba(143, 179, 255, 0.22);
    border-color: rgba(143, 179, 255, 0.4);
}

.curves__chip--custom {
    padding-right: 6px;
}

.curves__chip-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 140px;
}

.curves__chip-del {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.75);
    font-size: 10px;
    line-height: 1;
    cursor: pointer;
}

.curves__chip-del:hover {
    background: rgba(255, 120, 120, 0.35);
    color: #fff;
}

/* Light theme */
:root[data-theme="light"] .curves__presets-title,
:root[data-theme="light"] .curves__presets-cat {
    color: rgba(27, 39, 54, 0.55);
}

:root[data-theme="light"] .curves__chip {
    border-color: rgba(0, 0, 0, 0.14);
    background: rgba(0, 0, 0, 0.04);
    color: rgba(27, 39, 54, 0.9);
}

:root[data-theme="light"] .curves__chip:hover {
    background: rgba(47, 107, 216, 0.14);
    border-color: rgba(47, 107, 216, 0.35);
}
</style>

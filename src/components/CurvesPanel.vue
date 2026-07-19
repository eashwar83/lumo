<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
    type CurveChannel,
    type CurvePoint,
    type Curves,
    defaultCurves,
    identityPoints,
    makeCurveEvaluator,
    parseCurves,
    serializeCurves,
} from "../utils/curves";
import type { VideoEnhancementsController } from "../composables/useVideoEnhancements";

const props = defineProps<{
    enhancements: VideoEnhancementsController;
    visible: boolean;
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
    pts.splice(index, 1);
    emitChange();
};

const resetChannel = () => {
    curves[channel.value] = identityPoints();
    emitChange();
};

const resetAll = () => {
    const fresh = defaultCurves();
    curves.rgb = fresh.rgb;
    curves.r = fresh.r;
    curves.g = fresh.g;
    curves.b = fresh.b;
    emitChange();
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

.curves__editor {
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
</style>

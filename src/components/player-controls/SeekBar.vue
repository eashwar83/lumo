<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

const props = defineProps<{
    duration: number;
    mediaPath?: string;
    progressPercent: number;
    bufferedPercent: number;
    formatTime: (seconds: number) => string;
    controlsVisible?: boolean;
}>();

const emit = defineEmits<{
    (e: "seek", position: number): void;
}>();

const progressAreaRef = ref<HTMLElement | null>(null);
const isDragging = ref(false);
const activePointerId = ref<number | null>(null);
const dragPercent = ref<number | null>(null);

const showHoverTime = ref(false);
const hoverTime = ref(0);
const hoverPercent = ref(0);

// --- Seek-bar thumbnail preview -------------------------------------------
const thumbUrl = ref<string | null>(null);
// Bucket the hover fraction so we only fetch when crossing to a new frame.
const THUMB_BUCKETS = 120;
let lastThumbBucket = -1;
let thumbTimer: number | null = null;
const thumbCache = new Map<number, string | null>();

const isLocalPath = (p?: string): boolean =>
    !!p && !/^(https?|rtsp|rtmp|smb|webdav):\/\//i.test(p);

const fetchThumb = async (fraction: number) => {
    const path = props.mediaPath;
    if (!isLocalPath(path)) {
        thumbUrl.value = null;
        return;
    }
    const bucket = Math.max(
        0,
        Math.min(THUMB_BUCKETS - 1, Math.round(fraction * (THUMB_BUCKETS - 1))),
    );
    if (bucket === lastThumbBucket) return;
    lastThumbBucket = bucket;
    if (thumbCache.has(bucket)) {
        thumbUrl.value = thumbCache.get(bucket) ?? null;
        return;
    }
    try {
        const url = await invoke<string | null>("get_seek_thumbnail", {
            path,
            fraction: bucket / (THUMB_BUCKETS - 1),
        });
        thumbCache.set(bucket, url ?? null);
        // Only apply if the user is still hovering this bucket.
        if (lastThumbBucket === bucket) thumbUrl.value = url ?? null;
    } catch {
        thumbUrl.value = null;
    }
};

const scheduleThumb = (fraction: number) => {
    if (thumbTimer) window.clearTimeout(thumbTimer);
    thumbTimer = window.setTimeout(() => {
        thumbTimer = null;
        void fetchThumb(fraction);
    }, 40);
};

const clearThumb = () => {
    if (thumbTimer) {
        window.clearTimeout(thumbTimer);
        thumbTimer = null;
    }
    lastThumbBucket = -1;
    thumbUrl.value = null;
};

// Reset the per-file cache when the media changes.
watch(
    () => props.mediaPath,
    () => {
        thumbCache.clear();
        clearThumb();
    },
);

const clampRatio = (value: number) => Math.min(1, Math.max(0, value));

const getRatioFromClientX = (clientX: number) => {
    const container = progressAreaRef.value;
    if (!container) return 0;
    const rect = container.getBoundingClientRect();
    if (rect.width <= 0) return 0;
    return clampRatio((clientX - rect.left) / rect.width);
};

const updateHoverByClientX = (clientX: number) => {
    if (props.duration <= 0) {
        showHoverTime.value = false;
        return;
    }
    const ratio = getRatioFromClientX(clientX);
    hoverPercent.value = ratio * 100;
    hoverTime.value = ratio * props.duration;
    if (isDragging.value) {
        dragPercent.value = hoverPercent.value;
    }
    showHoverTime.value = true;
    scheduleThumb(ratio);
};

const updateHoverTime = (event: MouseEvent) => {
    if (isDragging.value) return;
    updateHoverByClientX(event.clientX);
};

const hideHoverTime = () => {
    if (isDragging.value) return;
    showHoverTime.value = false;
    clearThumb();
};

const stopDragging = (event?: PointerEvent) => {
    if (event && progressAreaRef.value?.hasPointerCapture(event.pointerId)) {
        progressAreaRef.value.releasePointerCapture(event.pointerId);
    }
    activePointerId.value = null;
    isDragging.value = false;
    dragPercent.value = null;
    window.removeEventListener("pointermove", onWindowPointerMove);
    window.removeEventListener("pointerup", onWindowPointerUp);
    window.removeEventListener("pointercancel", onWindowPointerCancel);
};

const onPointerDown = (event: PointerEvent) => {
    if (event.button !== 0 || props.duration <= 0) return;
    activePointerId.value = event.pointerId;
    isDragging.value = true;
    progressAreaRef.value?.setPointerCapture(event.pointerId);
    updateHoverByClientX(event.clientX);
    window.addEventListener("pointermove", onWindowPointerMove);
    window.addEventListener("pointerup", onWindowPointerUp);
    window.addEventListener("pointercancel", onWindowPointerCancel);
};

const onWindowPointerMove = (event: PointerEvent) => {
    if (!isDragging.value || event.pointerId !== activePointerId.value) return;
    updateHoverByClientX(event.clientX);
};

const onWindowPointerUp = (event: PointerEvent) => {
    if (!isDragging.value || event.pointerId !== activePointerId.value) return;
    updateHoverByClientX(event.clientX);
    emit("seek", hoverTime.value);
    stopDragging(event);
};

const onWindowPointerCancel = (event: PointerEvent) => {
    if (event.pointerId !== activePointerId.value) return;
    stopDragging(event);
    showHoverTime.value = false;
};

const onKeydown = () => {
    showHoverTime.value = false;
};

const displayProgressPercent = computed(() => {
    const percent =
        isDragging.value && dragPercent.value !== null
            ? dragPercent.value
            : props.progressPercent;
    return Math.min(100, Math.max(0, percent));
});

const displayBufferedPercent = computed(() => {
    const raw = Number.isFinite(props.bufferedPercent)
        ? props.bufferedPercent
        : 0;
    const buffered = Math.min(100, Math.max(0, raw));
    return Math.max(buffered, displayProgressPercent.value);
});

const bufferedAheadPercent = computed(() =>
    Math.max(0, displayBufferedPercent.value - displayProgressPercent.value),
);

watch(
    () => props.controlsVisible,
    (visible) => {
        if (visible === false && !isDragging.value) {
            hideHoverTime();
        }
    },
);

onMounted(() => {
    window.addEventListener("keydown", onKeydown);
});

onUnmounted(() => {
    stopDragging();
    clearThumb();
    window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
    <div
        ref="progressAreaRef"
        class="progress-area"
        data-window-no-drag
        :class="{ 'is-dragging': isDragging }"
        @pointerdown.prevent="onPointerDown"
        @mousemove="updateHoverTime"
        @mouseleave="hideHoverTime"
    >
        <div
            v-if="showHoverTime"
            class="seek-preview"
            :style="{ left: hoverPercent + '%' }"
        >
            <img
                v-if="thumbUrl"
                class="seek-preview__img"
                :src="thumbUrl"
                alt=""
                draggable="false"
            />
            <div class="seek-preview__time" :class="{ 'seek-preview__time--solo': !thumbUrl }">
                {{ formatTime(hoverTime) }}
            </div>
        </div>
        <div class="progress-bg">
            <div
                v-if="bufferedAheadPercent > 0.05"
                class="progress-buffer-ahead"
                :style="{
                    left: displayProgressPercent + '%',
                    width: bufferedAheadPercent + '%',
                }"
            ></div>
            <div
                class="progress-current"
                :style="{ width: displayProgressPercent + '%' }"
            ></div>
        </div>
        <div
            class="scrubber-head"
            :style="{ left: displayProgressPercent + '%' }"
        ></div>
    </div>
</template>

<style scoped>
.progress-area {
    height: 16px;
    display: flex;
    align-items: center;
    cursor: pointer;
    position: relative;
    touch-action: none;
}

.progress-bg {
    height: 3px;
    width: 100%;
    background: rgba(58, 58, 58, 0.92);
    position: relative;
    overflow: hidden;
    transition: height 0.1s;
}

.progress-area:hover .progress-bg {
    height: 5px;
}

.progress-area.is-dragging .progress-bg {
    height: 5px;
}

.progress-current {
    height: 100%;
    background: var(--progress-color);
    position: relative;
    z-index: 2;
}

.progress-buffer-ahead {
    position: absolute;
    top: 0;
    height: 100%;
    background: rgba(141, 141, 141, 0.88);
    z-index: 1;
    pointer-events: none;
}

.scrubber-head {
    position: absolute;
    left: 0;
    top: 50%;
    transform: translate(-50%, -50%) scale(0.6);
    width: 8px;
    height: 8px;
    background: var(--progress-color);
    border: 1px solid rgba(236, 243, 255, 0.95);
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(10, 22, 40, 0.2);
    opacity: 0;
    transition:
        transform 0.1s,
        opacity 0.1s;
    z-index: 3;
    pointer-events: none;
}

.progress-area:hover .scrubber-head {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1.25);
}

.progress-area.is-dragging .scrubber-head {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1.35);
}

.seek-preview {
    position: absolute;
    bottom: 14px;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    pointer-events: none;
    z-index: 5;
}

.seek-preview__img {
    width: 168px;
    height: auto;
    max-height: 108px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.25);
    background: #000;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.5);
    display: block;
}

.seek-preview__time {
    background: rgba(28, 28, 28, 0.9);
    color: #fff;
    font-size: 12px;
    padding: 3px 7px;
    border-radius: 6px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
}
</style>

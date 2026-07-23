<script setup lang="ts">
import { ref } from "vue";
import type { SplitCompareController } from "../composables/useSplitCompare";

// The draggable handle for the before/after wipe. The line itself is drawn by
// the shader (so it sits exactly on the pixel boundary); this is just the grab
// target and the labels.

const props = defineProps<{ compare: SplitCompareController }>();

const rootRef = ref<HTMLElement | null>(null);
const dragging = ref(false);
let pointerId: number | null = null;

const positionFromEvent = (clientX: number) => {
    const rect = rootRef.value?.getBoundingClientRect();
    if (!rect || rect.width <= 0) return props.compare.position.value;
    return (clientX - rect.left) / rect.width;
};

const onPointerMove = (event: PointerEvent) => {
    if (event.pointerId !== pointerId) return;
    props.compare.setPosition(positionFromEvent(event.clientX));
};

const endDrag = () => {
    pointerId = null;
    dragging.value = false;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", endDrag);
    window.removeEventListener("pointercancel", endDrag);
    // Bake the final value into the shader so it survives a reload.
    props.compare.commitPosition();
};

const onPointerDown = (event: PointerEvent) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    pointerId = event.pointerId;
    dragging.value = true;
    props.compare.setPosition(positionFromEvent(event.clientX));
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", endDrag);
    window.addEventListener("pointercancel", endDrag);
};
</script>

<template>
    <div
        v-if="props.compare.isEnabled.value"
        ref="rootRef"
        class="split"
        data-window-no-drag
    >
        <span class="split__tag split__tag--before">Original</span>
        <span class="split__tag split__tag--after">Enhanced</span>

        <div
            class="split__handle"
            :class="{ 'split__handle--dragging': dragging }"
            :style="{ left: props.compare.position.value * 100 + '%' }"
            role="slider"
            :aria-valuenow="Math.round(props.compare.position.value * 100)"
            aria-valuemin="0"
            aria-valuemax="100"
            aria-label="Before / after split position"
            tabindex="0"
            @pointerdown="onPointerDown"
            @keydown.left.prevent="
                props.compare.setPosition(props.compare.position.value - 0.02);
                props.compare.commitPosition();
            "
            @keydown.right.prevent="
                props.compare.setPosition(props.compare.position.value + 0.02);
                props.compare.commitPosition();
            "
        >
            <span class="split__grip">
                <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                    <path d="M9 6l-5 6 5 6zM15 6l5 6-5 6z" />
                </svg>
            </span>
        </div>
    </div>
</template>

<style scoped>
.split {
    position: fixed;
    inset: 0;
    /* Above the video, below the control chrome. Only the handle takes input. */
    z-index: 95;
    pointer-events: none;
    user-select: none;
    -webkit-user-select: none;
}

.split__tag {
    position: absolute;
    top: 54px;
    padding: 4px 10px;
    border-radius: 6px;
    background: rgba(12, 14, 18, 0.72);
    color: #fff;
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
}

.split__tag--before {
    left: 18px;
}

.split__tag--after {
    right: 18px;
}

/* Wide invisible grab strip centred on the shader-drawn line. */
.split__handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 34px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: ew-resize;
    pointer-events: auto;
}

.split__grip {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: rgba(12, 14, 18, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.55);
    color: #fff;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.5);
    transition: transform 0.12s ease, background-color 0.12s ease;
}

.split__handle:hover .split__grip,
.split__handle--dragging .split__grip {
    background: rgba(28, 32, 40, 0.92);
    transform: scale(1.1);
}

.split__handle:focus-visible .split__grip {
    outline: 2px solid #fff;
    outline-offset: 2px;
}

.split__grip svg {
    width: 17px;
    height: 17px;
}
</style>

<script setup lang="ts">
import {
    computed,
    nextTick,
    onBeforeUnmount,
    onMounted,
    onUpdated,
    ref,
    useAttrs,
} from "vue";

defineOptions({ inheritAttrs: false });

const props = withDefaults(
    defineProps<{
        minThumb?: number;
        alwaysVisible?: boolean;
    }>(),
    {
        minThumb: 28,
        alwaysVisible: false,
    },
);

const viewportRef = ref<HTMLElement | null>(null);
const contentRef = ref<HTMLElement | null>(null);
const trackRef = ref<HTMLElement | null>(null);
const thumbRef = ref<HTMLElement | null>(null);

const thumbHeight = ref(0);
const thumbTop = ref(0);
const hasOverflow = ref(false);
const isHovering = ref(false);
const isDragging = ref(false);

let resizeObserver: ResizeObserver | null = null;
let rafId = 0;
let dragStartY = 0;
let startScrollTop = 0;
let scrollRatio = 1;

const showThumb = computed(
    () => hasOverflow.value && (props.alwaysVisible || isHovering.value || isDragging.value),
);

const attrs = useAttrs();
const rootAttrs = computed(() => {
    const { class: className, ...rest } = attrs;
    return {
        ...rest,
        class: ["scroll-area", className],
    };
});

const updateThumb = () => {
    const viewport = viewportRef.value;
    const track = trackRef.value;
    if (!viewport) return;

    const { scrollHeight, clientHeight, scrollTop } = viewport;
    const maxScroll = Math.max(scrollHeight - clientHeight, 0);
    hasOverflow.value = maxScroll > 1;

    if (!hasOverflow.value) {
        thumbHeight.value = 0;
        thumbTop.value = 0;
        return;
    }

    const trackHeight = track?.clientHeight ?? clientHeight;
    const ratio = clientHeight / scrollHeight;
    const nextThumbHeight = Math.max(trackHeight * ratio, props.minThumb);
    const maxThumbTop = Math.max(trackHeight - nextThumbHeight, 0);

    thumbHeight.value = nextThumbHeight;
    thumbTop.value = maxScroll
        ? (scrollTop / maxScroll) * maxThumbTop
        : 0;
};

const scheduleUpdate = () => {
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
        rafId = 0;
        updateThumb();
    });
};

const onScroll = () => scheduleUpdate();

const onThumbPointerDown = (event: PointerEvent) => {
    const viewport = viewportRef.value;
    const track = trackRef.value;
    if (!viewport) return;

    event.preventDefault();
    isDragging.value = true;
    dragStartY = event.clientY;
    startScrollTop = viewport.scrollTop;

    const maxScroll = Math.max(viewport.scrollHeight - viewport.clientHeight, 0);
    const trackHeight = track?.clientHeight ?? viewport.clientHeight;
    const maxThumbTop = Math.max(trackHeight - thumbHeight.value, 1);
    scrollRatio = maxThumbTop ? maxScroll / maxThumbTop : 1;

    thumbRef.value?.setPointerCapture?.(event.pointerId);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp, { once: true });
};

const onPointerMove = (event: PointerEvent) => {
    const viewport = viewportRef.value;
    if (!viewport) return;
    const delta = event.clientY - dragStartY;
    viewport.scrollTop = startScrollTop + delta * scrollRatio;
};

const onPointerUp = () => {
    isDragging.value = false;
    window.removeEventListener("pointermove", onPointerMove);
};

const onTrackPointerDown = (event: PointerEvent) => {
    const viewport = viewportRef.value;
    const track = trackRef.value;
    if (!viewport || !track) return;

    const rect = track.getBoundingClientRect();
    const offset = event.clientY - rect.top - thumbHeight.value / 2;
    const maxThumbTop = Math.max(rect.height - thumbHeight.value, 1);
    const clampedTop = Math.max(0, Math.min(offset, maxThumbTop));
    const maxScroll = Math.max(viewport.scrollHeight - viewport.clientHeight, 0);
    viewport.scrollTop = maxScroll ? (clampedTop / maxThumbTop) * maxScroll : 0;
};

onMounted(async () => {
    await nextTick();
    const viewport = viewportRef.value;
    const content = contentRef.value;
    if (!viewport || !content) return;

    viewport.addEventListener("scroll", onScroll, { passive: true });
    updateThumb();

    if (typeof ResizeObserver !== "undefined") {
        resizeObserver = new ResizeObserver(scheduleUpdate);
        resizeObserver.observe(viewport);
        resizeObserver.observe(content);
    }
});

onUpdated(() => scheduleUpdate());

onBeforeUnmount(() => {
    viewportRef.value?.removeEventListener("scroll", onScroll);
    resizeObserver?.disconnect();
    if (rafId) cancelAnimationFrame(rafId);
    window.removeEventListener("pointermove", onPointerMove);
});
</script>

<template>
    <div
        v-bind="rootAttrs"
        @pointerenter="isHovering = true"
        @pointerleave="isHovering = false"
    >
        <div ref="viewportRef" class="scroll-area__viewport">
            <div ref="contentRef" class="scroll-area__content">
                <slot />
            </div>
        </div>
        <div
            ref="trackRef"
            class="scroll-area__track"
            :class="{ 'scroll-area__track--visible': showThumb }"
            @pointerdown="onTrackPointerDown"
        >
            <div
                ref="thumbRef"
                class="scroll-area__thumb"
                :style="{
                    height: `${thumbHeight}px`,
                    transform: `translateY(${thumbTop}px)`,
                }"
                @pointerdown.stop="onThumbPointerDown"
            ></div>
        </div>
    </div>
</template>

<style scoped>
.scroll-area {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
}

.scroll-area__viewport {
    flex: 1;
    min-height: 0;
    overflow: auto;
    scrollbar-width: none;
    -ms-overflow-style: none;
    padding-right: 0;
}

.scroll-area__viewport::-webkit-scrollbar {
    width: 0;
    height: 0;
}

.scroll-area__content {
    min-height: 100%;
    display: flex;
    flex-direction: column;
}

.scroll-area__track {
    position: absolute;
    right: 2px;
    top: var(--scrollbar-track-top, 6px);
    bottom: var(--scrollbar-track-bottom, 6px);
    width: var(--scrollbar-size, 6px);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.2s ease;
}

.scroll-area__track--visible {
    opacity: 1;
    pointer-events: auto;
}


.scroll-area__thumb {
    position: absolute;
    right: 0;
    width: 100%;
    border-radius: 999px;
    background: var(--scrollbar-thumb, rgba(0, 0, 0, 0.28));
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.06);
    transition: background-color 0.2s ease, opacity 0.2s ease;
}

.scroll-area__track--visible .scroll-area__thumb:hover,
.scroll-area__track--visible .scroll-area__thumb:active {
    background: var(--scrollbar-thumb-hover, rgba(0, 0, 0, 0.4));
}

@media (prefers-color-scheme: dark) {
    :root:not([data-theme]) .scroll-area__thumb {
        background: var(--scrollbar-thumb, rgba(255, 255, 255, 0.24));
        box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08);
    }

    :root:not([data-theme]) .scroll-area__track--visible .scroll-area__thumb:hover,
    :root:not([data-theme]) .scroll-area__track--visible .scroll-area__thumb:active {
        background: var(--scrollbar-thumb-hover, rgba(255, 255, 255, 0.4));
    }
}

:root:is([data-theme="dark"], [data-theme="graphite"]) .scroll-area__thumb {
    background: var(--scrollbar-thumb, rgba(255, 255, 255, 0.24));
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08);
}

:root:is([data-theme="dark"], [data-theme="graphite"]) .scroll-area__track--visible .scroll-area__thumb:hover,
:root:is([data-theme="dark"], [data-theme="graphite"]) .scroll-area__track--visible .scroll-area__thumb:active {
    background: var(--scrollbar-thumb-hover, rgba(255, 255, 255, 0.4));
}
</style>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { AbRangeController } from "../composables/useAbRange";
import type { SkipMarkersController } from "../composables/useSkipMarkers";

// The A-B strip that appears above the player controls once an in point is
// marked. It owns the range readout, the loop toggle, the clip/GIF exports and
// the "save as intro/credits" actions that feed the skip-marker memory.

const props = defineProps<{
    ab: AbRangeController;
    skip: SkipMarkersController;
    formatTime: (seconds: number) => string;
    exporting: boolean;
    /** GIF export is capped; longer ranges disable that button. */
    gifMaxSeconds: number;
    /** False for network sources — both exports need a real file. */
    canExport: boolean;
    /** Video clips need a full ffmpeg; GIFs don't. */
    clipAvailable: boolean;
}>();

const emit = defineEmits<{
    (e: "export", payload: { asGif: boolean; gifWidth: number }): void;
}>();

const showSaveMenu = ref(false);

// GIF size. 0 = keep the source resolution. GIF has no interframe compression
// worth the name, so the file grows roughly with width squared — the labels
// double as the warning.
const GIF_SIZES = [
    { width: 480, label: "480" },
    { width: 720, label: "720" },
    { width: 1080, label: "1080" },
    { width: 0, label: "Full" },
] as const;
const gifWidth = ref<number>(720);

const rangeLabel = computed(() => {
    const { pointA, pointB } = props.ab;
    if (pointA.value === null) return "";
    if (pointB.value === null) {
        return `${props.formatTime(pointA.value)} → …`;
    }
    return `${props.formatTime(pointA.value)} → ${props.formatTime(pointB.value)}`;
});

const durationLabel = computed(() => {
    const seconds = props.ab.rangeLength.value;
    if (seconds <= 0) return "";
    return seconds < 60
        ? `${seconds.toFixed(1)}s`
        : props.formatTime(seconds);
});

// Mirrors the backend's pixel budget so the button disables before the user
// waits on an export that would be rejected.
const GIF_PIXEL_BUDGET = 1_600_000_000;
const GIF_FPS = 12;

const gifTooLong = computed(() => {
    const seconds = props.ab.rangeLength.value;
    if (seconds > props.gifMaxSeconds) return true;
    // "Full" can't be checked here — the source height is unknown until the
    // frames come out, so the backend re-checks and reports.
    if (gifWidth.value === 0) return false;
    const height = Math.round((gifWidth.value * 9) / 16);
    return gifWidth.value * height * GIF_FPS * Math.ceil(seconds) > GIF_PIXEL_BUDGET;
});

const onSaveIntro = () => {
    const { pointA, pointB } = props.ab;
    if (pointA.value === null || pointB.value === null) return;
    props.skip.saveIntro(pointA.value, pointB.value);
    showSaveMenu.value = false;
};

const onSaveCredits = () => {
    const { pointA } = props.ab;
    if (pointA.value === null) return;
    props.skip.saveCredits(pointA.value);
    showSaveMenu.value = false;
};

const onClearMarkers = () => {
    props.skip.clearMarkers();
    showSaveMenu.value = false;
};
</script>

<template>
    <transition name="clipbar-fade">
        <div v-if="props.ab.isActive.value" class="clipbar" data-window-no-drag>
            <span class="clipbar__badge">A-B</span>
            <span class="clipbar__range">{{ rangeLabel }}</span>
            <span v-if="durationLabel" class="clipbar__len">{{ durationLabel }}</span>

            <template v-if="!props.ab.hasRange.value">
                <span class="clipbar__hint">
                    Press <kbd>K</kbd> again at the end point
                </span>
            </template>

            <template v-else>
                <button
                    class="clipbar__btn"
                    type="button"
                    :class="{ 'clipbar__btn--on': props.ab.loopEnabled.value }"
                    :aria-pressed="props.ab.loopEnabled.value"
                    title="Loop this range"
                    @click="props.ab.setLoopEnabled(!props.ab.loopEnabled.value)"
                >
                    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                        <path d="M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z" />
                    </svg>
                    Loop
                </button>

                <button
                    class="clipbar__btn clipbar__btn--primary"
                    type="button"
                    :disabled="
                        props.exporting || !props.canExport || !props.clipAvailable
                    "
                    :title="
                        props.clipAvailable
                            ? 'Export this exact range as an MP4'
                            : 'Needs ffmpeg — install it or set its path in Settings → Advanced'
                    "
                    @click="emit('export', { asGif: false, gifWidth: 0 })"
                >
                    {{ props.exporting ? "Exporting…" : "Export clip" }}
                </button>

                <div class="clipbar__gif">
                    <button
                        class="clipbar__btn"
                        type="button"
                        :disabled="props.exporting || !props.canExport || gifTooLong"
                        :title="
                            gifTooLong
                                ? 'Too long for a GIF at this size — shorten the range or pick a smaller size'
                                : 'Export this range as a GIF'
                        "
                        @click="
                            emit('export', { asGif: true, gifWidth: gifWidth })
                        "
                    >
                        GIF
                    </button>
                    <div class="clipbar__sizes" role="group" aria-label="GIF size">
                        <button
                            v-for="size in GIF_SIZES"
                            :key="size.width"
                            class="clipbar__size"
                            :class="{
                                'clipbar__size--active': gifWidth === size.width,
                            }"
                            type="button"
                            :title="
                                size.width === 0
                                    ? 'Source resolution — large files, keep it short'
                                    : `${size.width}px wide`
                            "
                            @click="gifWidth = size.width"
                        >
                            {{ size.label }}
                        </button>
                    </div>
                </div>

                <div class="clipbar__menu-wrap">
                    <button
                        class="clipbar__btn"
                        type="button"
                        title="Save this range as the intro or credits for this folder"
                        @click="showSaveMenu = !showSaveMenu"
                    >
                        Skip markers ▾
                    </button>
                    <div v-if="showSaveMenu" class="clipbar__menu">
                        <button type="button" @click="onSaveIntro">
                            Save A→B as intro
                        </button>
                        <button type="button" @click="onSaveCredits">
                            Save A as credits start
                        </button>
                        <button
                            type="button"
                            :disabled="
                                !props.skip.hasIntroMarkers.value &&
                                !props.skip.hasCreditsMarker.value
                            "
                            @click="onClearMarkers"
                        >
                            Clear markers for this folder
                        </button>
                        <label class="clipbar__auto">
                            <input
                                type="checkbox"
                                :checked="props.skip.autoSkip.value"
                                @change="
                                    props.skip.setAutoSkip(
                                        ($event.target as HTMLInputElement).checked,
                                    )
                                "
                            />
                            Skip automatically
                        </label>
                    </div>
                </div>
            </template>

            <button
                class="clipbar__close"
                type="button"
                aria-label="Clear A-B range"
                @click="props.ab.clear()"
            >
                ✕
            </button>
        </div>
    </transition>
</template>

<style scoped>
.clipbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    margin: 0 auto 6px;
    width: fit-content;
    max-width: 100%;
    border-radius: 10px;
    background: rgba(18, 20, 24, 0.86);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #fff;
    font-size: 12px;
    user-select: none;
    -webkit-user-select: none;
}

.clipbar__badge {
    padding: 2px 6px;
    border-radius: 5px;
    background: var(--progress-color, #4a9eff);
    color: #0b0d10;
    font-size: 10.5px;
    font-weight: 800;
    letter-spacing: 0.04em;
}

.clipbar__range {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    white-space: nowrap;
}

.clipbar__len {
    color: rgba(255, 255, 255, 0.55);
    font-variant-numeric: tabular-nums;
}

.clipbar__hint {
    color: rgba(255, 255, 255, 0.55);
    white-space: nowrap;
}

.clipbar__hint kbd {
    padding: 1px 5px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.14);
    font-family: inherit;
    font-size: 11px;
}

.clipbar__btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.07);
    color: rgba(255, 255, 255, 0.88);
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    transition: background-color 0.15s ease, color 0.15s ease;
}

.clipbar__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.14);
    color: #fff;
}

.clipbar__btn:disabled {
    opacity: 0.4;
    cursor: default;
}

.clipbar__btn svg {
    width: 13px;
    height: 13px;
}

.clipbar__btn--on {
    background: color-mix(in srgb, var(--progress-color, #4a9eff) 30%, transparent);
    border-color: var(--progress-color, #4a9eff);
    color: #fff;
}

.clipbar__btn--primary {
    background: rgba(255, 255, 255, 0.9);
    border-color: transparent;
    color: #16181c;
}

.clipbar__btn--primary:hover:not(:disabled) {
    background: #fff;
    color: #16181c;
}

.clipbar__gif {
    display: flex;
    align-items: center;
    gap: 4px;
}

.clipbar__sizes {
    display: flex;
    gap: 1px;
    padding: 2px;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.07);
}

.clipbar__size {
    min-width: 30px;
    padding: 3px 5px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    font-size: 10.5px;
    font-weight: 700;
    cursor: pointer;
    transition: background-color 0.15s ease, color 0.15s ease;
}

.clipbar__size:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.1);
}

.clipbar__size--active {
    background: rgba(255, 255, 255, 0.9);
    color: #16181c;
}

.clipbar__menu-wrap {
    position: relative;
}

.clipbar__menu {
    position: absolute;
    bottom: calc(100% + 6px);
    right: 0;
    min-width: 210px;
    display: flex;
    flex-direction: column;
    padding: 5px;
    border-radius: 9px;
    background: rgba(24, 26, 31, 0.97);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    z-index: 10;
}

.clipbar__menu button {
    padding: 7px 9px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.88);
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
}

.clipbar__menu button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}

.clipbar__menu button:disabled {
    opacity: 0.4;
    cursor: default;
}

.clipbar__auto {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 9px;
    margin-top: 3px;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    font-size: 11.5px;
    color: rgba(255, 255, 255, 0.8);
    cursor: pointer;
}

.clipbar__close {
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.75);
    font-size: 11px;
    cursor: pointer;
}

.clipbar__close:hover {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
}

.clipbar-fade-enter-active,
.clipbar-fade-leave-active {
    transition: opacity 0.18s ease, transform 0.18s ease;
}

.clipbar-fade-enter-from,
.clipbar-fade-leave-to {
    opacity: 0;
    transform: translateY(6px);
}
</style>

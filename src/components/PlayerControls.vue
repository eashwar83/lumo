<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from "vue";
import type { MediaTrack } from "../types/media";
import type { SubtitleTarget } from "../composables/useSubtitleState";
import SeekBar from "./player-controls/SeekBar.vue";
import LeftControls from "./player-controls/LeftControls.vue";
import RightControls from "./player-controls/RightControls.vue";

const props = defineProps<{
    isPlaying: boolean;
    currentTime: number;
    duration: number;
    progressPercent: number;
    bufferedPercent: number;
    formatTime: (seconds: number) => string;
    controlsVisible?: boolean;
    isHidden: boolean;
    statusBadges: string[];
    currentSpeed: number;
    playbackRates: number[];
    showSpeedMenu: boolean;
    showSettingsMenu: boolean;
    audioDelay: number;
    subDelay: number;
    secondarySubDelay: number;
    brightness: number;
    contrast: number;
    saturation: number;
    gamma: number;
    hue: number;
    isLoopOne: boolean;
    audioTracks: MediaTrack[];
    showAudioMenu: boolean;
    subTracks: MediaTrack[];
    dualSubEnabled: boolean;
    secondarySubId: MediaTrack["id"];
    activeSubTarget: SubtitleTarget;
    showSubMenu: boolean;
    hasAudioTracks: boolean;
    hasSubTracks: boolean;
    isFullscreen: boolean;
}>();

const emit = defineEmits<{
    (e: "prev-track"): void;
    (e: "seek", position: number): void;
    (e: "toggle-play-pause"): void;
    (e: "stop-playback"): void;
    (e: "next-track"): void;
    (e: "toggle-menu", menuName: "audio" | "sub" | "speed" | "settings"): void;
    (e: "toggle-loop-one"): void;
    (e: "set-speed", rate: number): void;
    (e: "set-audio-delay", value: number): void;
    (e: "set-sub-delay-for-target", payload: { target: SubtitleTarget; value: number }): void;
    (e: "set-brightness", value: number): void;
    (e: "set-contrast", value: number): void;
    (e: "set-saturation", value: number): void;
    (e: "set-gamma", value: number): void;
    (e: "set-hue", value: number): void;
    (e: "select-audio", track: MediaTrack): void;
    (e: "select-sub-track", payload: { target: SubtitleTarget; track: MediaTrack }): void;
    (e: "set-active-sub-target", target: SubtitleTarget): void;
    (e: "toggle-dual-sub", enabled: boolean): void;
    (e: "add-external-audio"): void;
    (e: "add-external-sub"): void;
    (e: "toggle-fullscreen"): void;
}>();

const controlsViewportRef = ref<HTMLElement | null>(null);
const controlsMainRef = ref<HTMLElement | null>(null);
const controlsMainScale = ref(1);
const MIN_CONTROLS_SCALE = 0.5;
const SCALE_EXIT_BUFFER_PX = 6;
let controlsResizeObserver: ResizeObserver | null = null;
let controlsScaleRaf = 0;

const queueControlsScaleUpdate = () => {
    if (controlsScaleRaf) {
        cancelAnimationFrame(controlsScaleRaf);
    }
    controlsScaleRaf = requestAnimationFrame(() => {
        controlsScaleRaf = 0;
        updateControlsMainScale();
    });
};

const updateControlsMainScale = () => {
    const controlsViewport = controlsViewportRef.value;
    const controlsMain = controlsMainRef.value;
    if (!controlsViewport || !controlsMain) {
        controlsMainScale.value = 1;
        return;
    }
    const leftControls = controlsMain.querySelector<HTMLElement>(".controls-left");
    const rightControls = controlsMain.querySelector<HTMLElement>(".controls-right");
    if (!leftControls || !rightControls) {
        controlsMainScale.value = 1;
        return;
    }

    const measureGroupWidth = (group: HTMLElement) => {
        const groupStyle = window.getComputedStyle(group);
        const gap = Number.parseFloat(groupStyle.columnGap || groupStyle.gap || "0") || 0;
        const children = Array.from(group.children) as HTMLElement[];
        let total = 0;

        for (const child of children) {
            const rawWidth = child.offsetWidth;
            const childStyle = window.getComputedStyle(child);
            const marginLeft = Number.parseFloat(childStyle.marginLeft || "0") || 0;
            const marginRight = Number.parseFloat(childStyle.marginRight || "0") || 0;
            total += rawWidth + marginLeft + marginRight;
        }

        if (children.length > 1) {
            total += gap * (children.length - 1);
        }

        return total;
    };

    const availableWidth = controlsViewport.clientWidth;
    const requiredWidth = measureGroupWidth(leftControls) + measureGroupWidth(rightControls) + 8;

    if (availableWidth <= 0 || requiredWidth <= 0) {
        if (controlsMainScale.value !== 1) {
            controlsMainScale.value = 1;
        }
        return;
    }

    if (requiredWidth <= availableWidth) {
        if (
            controlsMainScale.value < 1 &&
            availableWidth < requiredWidth + SCALE_EXIT_BUFFER_PX
        ) {
            return;
        }
        if (controlsMainScale.value !== 1) {
            controlsMainScale.value = 1;
        }
        return;
    }

    const nextScale = Math.max(
        MIN_CONTROLS_SCALE,
        Math.min(1, availableWidth / requiredWidth),
    );
    const roundedNextScale = Math.round(nextScale * 1000) / 1000;
    if (Math.abs(roundedNextScale - controlsMainScale.value) >= 0.005) {
        controlsMainScale.value = roundedNextScale;
    }
};

onMounted(() => {
    nextTick(() => {
        queueControlsScaleUpdate();
        if (typeof ResizeObserver !== "undefined") {
            controlsResizeObserver = new ResizeObserver(queueControlsScaleUpdate);
            if (controlsViewportRef.value) {
                controlsResizeObserver.observe(controlsViewportRef.value);
            }
            const controlsMain = controlsMainRef.value;
            if (controlsMain) {
                const leftControls = controlsMain.querySelector<HTMLElement>(".controls-left");
                const rightControls = controlsMain.querySelector<HTMLElement>(".controls-right");
                if (leftControls) {
                    controlsResizeObserver.observe(leftControls);
                }
                if (rightControls) {
                    controlsResizeObserver.observe(rightControls);
                }
            }
        }
    });
});

onUnmounted(() => {
    if (controlsScaleRaf) {
        cancelAnimationFrame(controlsScaleRaf);
        controlsScaleRaf = 0;
    }
    controlsResizeObserver?.disconnect();
    controlsResizeObserver = null;
});
</script>

<template>
    <div
        class="player-controls ui-surface"
        :class="{ 'ui-hidden': props.isHidden }"
    >
        <div class="player-controls-content">
            <SeekBar
                :duration="duration"
                :progress-percent="progressPercent"
                :buffered-percent="bufferedPercent"
                :format-time="formatTime"
                :controls-visible="controlsVisible"
                @seek="emit('seek', $event)"
            />
            <div ref="controlsViewportRef" class="controls-main-viewport">
                <div
                    ref="controlsMainRef"
                    class="controls-main"
                    :style="{
                        '--controls-main-scale': controlsMainScale.toString(),
                    }"
                >
                    <LeftControls
                        :is-playing="isPlaying"
                        :current-time="currentTime"
                        :duration="duration"
                        :format-time="formatTime"
                        :badges="props.statusBadges"
                        @prev-track="emit('prev-track')"
                        @toggle-play-pause="emit('toggle-play-pause')"
                        @stop-playback="emit('stop-playback')"
                        @next-track="emit('next-track')"
                    />
                    <RightControls
                        :current-speed="currentSpeed"
                        :playback-rates="playbackRates"
                        :show-speed-menu="showSpeedMenu"
                        :show-settings-menu="showSettingsMenu"
                        :audio-delay="audioDelay"
                        :sub-delay="subDelay"
                        :secondary-sub-delay="secondarySubDelay"
                        :brightness="brightness"
                        :contrast="contrast"
                        :saturation="saturation"
                        :gamma="gamma"
                        :hue="hue"
                        :is-loop-one="isLoopOne"
                        :audio-tracks="audioTracks"
                        :show-audio-menu="showAudioMenu"
                        :sub-tracks="subTracks"
                        :dual-sub-enabled="dualSubEnabled"
                        :secondary-sub-id="secondarySubId"
                        :active-sub-target="activeSubTarget"
                        :show-sub-menu="showSubMenu"
                        :has-audio-tracks="hasAudioTracks"
                        :has-sub-tracks="hasSubTracks"
                        :is-fullscreen="isFullscreen"
                        @toggle-menu="emit('toggle-menu', $event)"
                        @toggle-loop-one="emit('toggle-loop-one')"
                        @set-speed="emit('set-speed', $event)"
                        @set-audio-delay="emit('set-audio-delay', $event)"
                        @set-sub-delay-for-target="
                            emit('set-sub-delay-for-target', $event)
                        "
                        @set-brightness="emit('set-brightness', $event)"
                        @set-contrast="emit('set-contrast', $event)"
                        @set-saturation="emit('set-saturation', $event)"
                        @set-gamma="emit('set-gamma', $event)"
                        @set-hue="emit('set-hue', $event)"
                        @select-audio="emit('select-audio', $event)"
                        @select-sub-track="emit('select-sub-track', $event)"
                        @set-active-sub-target="emit('set-active-sub-target', $event)"
                        @toggle-dual-sub="emit('toggle-dual-sub', $event)"
                        @add-external-audio="emit('add-external-audio')"
                        @add-external-sub="emit('add-external-sub')"
                        @toggle-fullscreen="emit('toggle-fullscreen')"
                    />
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.player-controls {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    --controls-row-height: 35px;
    --seekbar-height: 16px;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.7));
    padding: 0 12px 8px 12px;
    transition: opacity 0.3s;
    z-index: 100;
}

.ui-hidden {
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
}

.player-controls-content {
    color: #fff;
}

.player-controls-content :deep(.controls-main-viewport) {
    width: 100%;
}

.player-controls-content :deep(.controls-main) {
    --controls-main-scale: 1;
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 35px;
    width: calc(100% / var(--controls-main-scale));
    min-width: 0;
    transform: scale(var(--controls-main-scale));
    transform-origin: left bottom;
}

.player-controls-content :deep(.controls-left),
.player-controls-content :deep(.controls-right) {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
}

.player-controls-content :deep(.icon-button--player) {
    width: 32px;
    height: 32px;
    padding: 1px;
}

.player-controls-content :deep(.icon-button--lg) {
    width: 32px;
    height: 32px;
}

.player-controls-content :deep(.icon-button--lg svg) {
    width: 32px;
    height: 32px;
}

.player-controls-content :deep(.time-display) {
    color: white;
    font-size: 13px;
    margin-left: 8px;
    user-select: none;
    font-variant-numeric: tabular-nums;
    font-feature-settings: "tnum";
}

.player-controls-content :deep(.separator) {
    margin: 0 4px;
}

.player-controls-content :deep(.track-menu-container) {
    position: relative;
    display: flex;
    align-items: center;
    padding: 2px;
    border-radius: 8px;
    transition:
        background-color 0.2s ease,
        box-shadow 0.2s ease;
}

.player-controls-content :deep(.track-menu-container:hover) {
    background: rgba(255, 255, 255, 0.08);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.player-controls-content :deep(.track-menu-container .icon-button--player) {
    border-radius: 6px;
}

.player-controls-content :deep(.track-menu-container .icon-button--player:focus-visible) {
    outline: 2px solid rgba(255, 255, 255, 0.5);
    outline-offset: 2px;
}

.player-controls-content :deep(.subtitle-icon) {
    width: 24px;
    height: 24px;
}

.player-controls-content :deep(.loop-toggle--active) {
    color: #8fb3ff;
}
</style>

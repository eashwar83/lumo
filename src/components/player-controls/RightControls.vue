<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { MediaTrack } from "../../types/media";
import type { SubtitleTarget } from "../../composables/useSubtitleState";
import ControlSlider from "./ControlSlider.vue";

const props = defineProps<{
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
    globalColorAdjustmentsEnabled: boolean;
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
    (e: "set-global-color-adjustments-enabled", enabled: boolean): void;
    (e: "select-audio", track: MediaTrack): void;
    (e: "select-sub-track", payload: { target: SubtitleTarget; track: MediaTrack }): void;
    (e: "set-active-sub-target", target: SubtitleTarget): void;
    (e: "toggle-dual-sub", enabled: boolean): void;
    (e: "add-external-audio"): void;
    (e: "add-external-sub"): void;
    (e: "toggle-fullscreen"): void;
}>();

const isNativePipPlatform =
    typeof navigator !== "undefined" &&
    /\b(mac|darwin|windows)\b/i.test(navigator.userAgent);
const isMacPlatform =
    typeof navigator !== "undefined" && /\b(mac|darwin)\b/i.test(navigator.userAgent);
const isWindowsPlatform =
    typeof navigator !== "undefined" && /\bwindows\b/i.test(navigator.userAgent);

const isPipEnabled = ref(false);
const isTogglingPip = ref(false);
let unlistenNativePipChanged: (() => void) | null = null;

const isSameTrackId = (left: MediaTrack["id"], right: MediaTrack["id"]) =>
    String(left) === String(right);
const activeSubTarget = computed(() => props.activeSubTarget);

const activeTrackId = computed<MediaTrack["id"]>(() =>
    props.activeSubTarget === "primary"
        ? props.subTracks.find((track) => track.selected)?.id ?? 0
        : props.secondarySubId,
);

const primaryTrackId = computed<MediaTrack["id"]>(
    () => props.subTracks.find((track) => track.selected)?.id ?? 0,
);

const isTrackDisabled = (track: MediaTrack) => {
    if (!props.dualSubEnabled) return false;
    if (String(track.id) === "0") return false;
    if (props.activeSubTarget === "primary") {
        return isSameTrackId(track.id, props.secondarySubId);
    }
    return isSameTrackId(track.id, primaryTrackId.value);
};

const isTrackSelectedByOtherTarget = (track: MediaTrack) => {
    if (!props.dualSubEnabled) return false;
    if (String(track.id) === "0") return false;
    if (props.activeSubTarget === "primary") {
        return isSameTrackId(track.id, props.secondarySubId);
    }
    return isSameTrackId(track.id, primaryTrackId.value);
};

const onSelectSubTrack = (track: MediaTrack) => {
    emit("select-sub-track", { target: props.activeSubTarget, track });
};

const onChangeActiveSubDelay = (value: number) => {
    emit("set-sub-delay-for-target", {
        target: props.activeSubTarget,
        value,
    });
};

const onResetActiveSubDelay = () => {
    onChangeActiveSubDelay(0);
};

const refreshPipState = async () => {
    if (!isNativePipPlatform) return;
    try {
        isPipEnabled.value = await invoke<boolean>("is_native_pip_enabled");
    } catch {
        // Ignore in non-Tauri environments.
    }
};

const onTogglePip = async () => {
    if (!isNativePipPlatform) return;
    if (isTogglingPip.value) return;
    isTogglingPip.value = true;
    try {
        const nextValue = !isPipEnabled.value;
        await invoke("set_native_pip_enabled", { enabled: nextValue });
        isPipEnabled.value = nextValue;
    } finally {
        isTogglingPip.value = false;
    }
};

onMounted(async () => {
    await refreshPipState();
    if (!isNativePipPlatform) return;
    try {
        unlistenNativePipChanged = await listen<boolean>("native-pip-changed", (event) => {
            isPipEnabled.value = Boolean(event.payload);
        });
    } catch {
        // Ignore in non-Tauri environments.
    }
});

onUnmounted(() => {
    unlistenNativePipChanged?.();
    unlistenNativePipChanged = null;
});
</script>

<template>
    <div class="controls-right">
        <div class="track-menu-container">
            <button
                class="icon-button icon-button--player"
                @click.stop="emit('toggle-menu', 'speed')"
                title="Playback Speed"
            >
                {{ currentSpeed }}x
            </button>

            <transition name="fade-up">
                <div v-if="showSpeedMenu" class="track-menu">
                    <div class="track-menu__header">Playback Speed</div>
                    <div class="track-menu__list">
                        <button
                            v-for="rate in playbackRates"
                            :key="rate"
                            class="track-menu__item"
                            :class="{
                                'track-menu__item--active':
                                    currentSpeed === rate,
                            }"
                            type="button"
                            @click="emit('set-speed', rate)"
                        >
                            <span class="track-menu__check">
                                <svg
                                    v-if="currentSpeed === rate"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                >
                                    <path
                                        d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"
                                    />
                                </svg>
                            </span>
                            <span class="track-menu__text">{{
                                rate === 1 ? "Normal" : rate + "x"
                            }}</span>
                        </button>
                    </div>
                </div>
            </transition>
        </div>

        <div class="track-menu-container">
            <button
                class="icon-button icon-button--player loop-toggle"
                :class="{ 'loop-toggle--active': isLoopOne }"
                @click.stop="emit('toggle-loop-one')"
                title="Loop One"
                :aria-pressed="isLoopOne"
            >
                <svg viewBox="0 0 24 24" fill="currentColor">
                    <path
                        d="M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4zm-4-2V9h-2v6h2z"
                    />
                </svg>
            </button>
        </div>

        <div class="track-menu-container">
            <button
                class="icon-button icon-button--player"
                @click.stop="emit('toggle-menu', 'audio')"
                title="Audio Tracks"
            >
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"
                    />
                </svg>
            </button>

            <transition name="fade-up">
                <div
                    v-if="showAudioMenu"
                    class="track-menu track-menu--wide"
                >
                    <div class="track-menu__header">
                        <span>Audio</span>
                        <button
                            class="icon-button track-menu__header-action"
                            type="button"
                            title="Add external audio track"
                            aria-label="Add external audio track"
                            @click.stop="emit('add-external-audio')"
                        >
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                <path
                                    d="M12 5v14M5 12h14"
                                    stroke-width="2"
                                    stroke-linecap="round"
                                />
                            </svg>
                        </button>
                    </div>
                    <div class="track-menu__list">
                        <button
                            v-for="track in audioTracks"
                            :key="track.id"
                            class="track-menu__item"
                            :class="{ 'track-menu__item--active': track.selected }"
                            type="button"
                            @click="emit('select-audio', track)"
                        >
                            <span class="track-menu__check">
                                <svg
                                    v-if="track.selected"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                >
                                    <path
                                        d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"
                                    />
                                </svg>
                            </span>
                            <span class="track-menu__text">
                                {{ track.title }}
                            </span>
                        </button>
                    </div>
                    <div v-if="props.hasAudioTracks" class="track-menu__footer">
                        <ControlSlider
                            label="Delay"
                            :value="audioDelay"
                            :min="-5"
                            :max="5"
                            :step="0.1"
                            unit="s"
                            :show-sign="true"
                            :precision="1"
                            @change="emit('set-audio-delay', $event)"
                            @reset="emit('set-audio-delay', 0)"
                        />
                    </div>
                </div>
            </transition>
        </div>

        <div class="track-menu-container">
            <button
                class="icon-button icon-button--player"
                @click.stop="emit('toggle-menu', 'sub')"
                title="Subtitles"
            >
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    class="subtitle-icon"
                    viewBox="0 -960 960 960"
                    fill="currentColor"
                >
                    <path
                        d="M240-320h320v-80H240v80Zm400 0h80v-80h-80v80ZM240-480h80v-80h-80v80Zm160 0h320v-80H400v80ZM160-160q-33 0-56.5-23.5T80-240v-480q0-33 23.5-56.5T160-800h640q33 0 56.5 23.5T880-720v480q0 33-23.5 56.5T800-160H160Zm0-80h640v-480H160v480Zm0 0v-480 480Z"
                    />
                </svg>
            </button>

            <transition name="fade-up">
                <div
                    v-if="showSubMenu"
                    class="track-menu track-menu--wide"
                >
                    <div class="track-menu__header">
                        <span>Subtitle</span>
                        <div class="track-menu__header-actions">
                            <button
                                class="track-menu__mode-toggle"
                                type="button"
                                :aria-pressed="dualSubEnabled"
                                :title="dualSubEnabled ? 'Dual subtitles' : 'Single subtitle'"
                                @click.stop="emit('toggle-dual-sub', !dualSubEnabled)"
                            >
                                <span class="track-menu__mode-label">Dual</span>
                                <span
                                    class="track-menu__mode-switch"
                                    :class="{ 'track-menu__mode-switch--on': dualSubEnabled }"
                                >
                                    <span class="track-menu__mode-thumb"></span>
                                </span>
                            </button>
                            <button
                                class="icon-button track-menu__header-action"
                                type="button"
                                title="Add external subtitles"
                                aria-label="Add external subtitles"
                                @click.stop="emit('add-external-sub')"
                            >
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <path
                                        d="M12 5v14M5 12h14"
                                        stroke-width="2"
                                        stroke-linecap="round"
                                    />
                                </svg>
                            </button>
                        </div>
                    </div>
                    <div class="track-menu__list">
                        <div v-if="dualSubEnabled" class="track-menu__sub-targets">
                            <button
                                class="track-menu__sub-target"
                                :class="{
                                    'track-menu__sub-target--active':
                                        activeSubTarget === 'primary',
                                }"
                                type="button"
                                @click="emit('set-active-sub-target', 'primary')"
                            >
                                PRIMARY
                            </button>
                            <button
                                class="track-menu__sub-target"
                                :class="{
                                    'track-menu__sub-target--active':
                                        activeSubTarget === 'secondary',
                                }"
                                type="button"
                                @click="emit('set-active-sub-target', 'secondary')"
                            >
                                SECONDARY
                            </button>
                        </div>
                        <div class="track-menu__section">
                            <button
                                v-for="track in subTracks"
                                :key="track.id"
                                class="track-menu__item"
                                :class="{
                                    'track-menu__item--active': isSameTrackId(
                                        track.id,
                                        activeTrackId,
                                    ) && !isTrackDisabled(track),
                                    'track-menu__item--disabled': isTrackDisabled(track),
                                }"
                                type="button"
                                :disabled="isTrackDisabled(track)"
                                @click="onSelectSubTrack(track)"
                            >
                                <span class="track-menu__check">
                                    <svg
                                        v-if="
                                            isSameTrackId(track.id, activeTrackId) ||
                                            isTrackSelectedByOtherTarget(track)
                                        "
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"
                                        />
                                    </svg>
                                </span>
                                <span class="track-menu__text">
                                    {{ track.title }}
                                </span>
                            </button>
                        </div>
                    </div>
                    <div v-if="props.hasSubTracks" class="track-menu__footer">
                        <ControlSlider
                            :label="dualSubEnabled ? 'Delay' : 'Delay'"
                            :value="
                                dualSubEnabled
                                    ? activeSubTarget === 'primary'
                                        ? subDelay
                                        : secondarySubDelay
                                    : subDelay
                            "
                            :min="-10"
                            :max="10"
                            :step="0.1"
                            unit="s"
                            :show-sign="true"
                            :precision="1"
                            @change="onChangeActiveSubDelay"
                            @reset="onResetActiveSubDelay"
                        />
                    </div>
                </div>
            </transition>
        </div>

        <div class="track-menu-container">
            <button
                class="icon-button icon-button--player"
                @click.stop="emit('toggle-menu', 'settings')"
                title="Video"
            >
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    class="settings-icon"
                    height="24px"
                    viewBox="0 -960 960 960"
                    width="24px"
                    fill="currentColor"
                >
                    <path
                        d="M480-160H160q-33 0-56.5-23.5T80-240v-480q0-33 23.5-56.5T160-800h640q33 0 56.5 23.5T880-720v200h-80v-200H160v480h320v80ZM380-300v-360l280 180-280 180ZM714-40l-12-60q-12-5-22.5-10.5T658-124l-58 18-40-68 46-40q-2-14-2-26t2-26l-46-40 40-68 58 18q11-8 21.5-13.5T702-380l12-60h80l12 60q12 5 22.5 11t21.5 15l58-20 40 70-46 40q2 12 2 25t-2 25l46 40-40 68-58-18q-11 8-21.5 13.5T806-100l-12 60h-80Zm40-120q33 0 56.5-23.5T834-240q0-33-23.5-56.5T754-320q-33 0-56.5 23.5T674-240q0 33 23.5 56.5T754-160Z"
                    />
                </svg>
            </button>

            <transition name="fade-up">
                <div v-if="showSettingsMenu" class="track-menu track-menu--settings">
                    <div class="track-menu__header">
                        <span>Video</span>
                        <div class="track-menu__header-actions">
                            <button
                                class="track-menu__mode-toggle"
                                type="button"
                                :aria-pressed="globalColorAdjustmentsEnabled"
                                :title="
                                    globalColorAdjustmentsEnabled
                                        ? 'Global apply enabled'
                                        : 'Enable global apply'
                                "
                                @click.stop="
                                    emit(
                                        'set-global-color-adjustments-enabled',
                                        !globalColorAdjustmentsEnabled,
                                    )
                                "
                            >
                                <span class="track-menu__mode-label">Global</span>
                                <span
                                    class="track-menu__mode-switch"
                                    :class="{
                                        'track-menu__mode-switch--on':
                                            globalColorAdjustmentsEnabled,
                                    }"
                                >
                                    <span class="track-menu__mode-thumb"></span>
                                </span>
                            </button>
                        </div>
                    </div>
                    <div class="track-menu__list track-menu__list--settings">
                        <ControlSlider
                            label="Brightness"
                            :value="brightness"
                            :min="-100"
                            :max="100"
                            :step="1"
                            unit="%"
                            :show-sign="true"
                            :precision="0"
                            @change="emit('set-brightness', $event)"
                            @reset="emit('set-brightness', 0)"
                        />
                        <ControlSlider
                            label="Contrast"
                            :value="contrast"
                            :min="-100"
                            :max="100"
                            :step="1"
                            unit="%"
                            :show-sign="true"
                            :precision="0"
                            @change="emit('set-contrast', $event)"
                            @reset="emit('set-contrast', 0)"
                        />
                        <ControlSlider
                            label="Saturation"
                            :value="saturation"
                            :min="-100"
                            :max="100"
                            :step="1"
                            unit="%"
                            :show-sign="true"
                            :precision="0"
                            @change="emit('set-saturation', $event)"
                            @reset="emit('set-saturation', 0)"
                        />
                        <ControlSlider
                            label="Gamma"
                            :value="gamma"
                            :min="-100"
                            :max="100"
                            :step="1"
                            unit="%"
                            :show-sign="true"
                            :precision="0"
                            @change="emit('set-gamma', $event)"
                            @reset="emit('set-gamma', 0)"
                        />
                        <ControlSlider
                            label="Hue"
                            :value="hue"
                            :min="-100"
                            :max="100"
                            :step="1"
                            unit="%"
                            :show-sign="true"
                            :precision="0"
                            @change="emit('set-hue', $event)"
                            @reset="emit('set-hue', 0)"
                        />
                    </div>
                </div>
            </transition>
        </div>

        <button
            v-if="isNativePipPlatform"
            class="icon-button icon-button--player icon-button--lg pip-toggle"
            :class="{ 'pip-toggle--active': isPipEnabled }"
            type="button"
            :disabled="isTogglingPip"
            :aria-disabled="isTogglingPip"
            :aria-pressed="isPipEnabled"
            :title="isPipEnabled ? 'Exit Picture in Picture' : 'Picture in Picture'"
            :aria-label="isPipEnabled ? 'Exit picture in picture' : 'Enter picture in picture'"
            @click.stop="onTogglePip"
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 -960 960 960"
                fill="currentColor"
                aria-hidden="true"
            >
                <path
                    v-if="isPipEnabled"
                    d="M160-160q-33 0-56.5-23.5T80-240v-280h80v280h640v-480H440v-80h360q33 0 56.5 23.5T880-720v480q0 33-23.5 56.5T800-160H160Zm523-140 57-57-124-123h104v-80H480v240h80v-103l123 123ZM80-600v-200h280v200H80Zm400 120Z"
                />
                <path
                    v-else
                    d="M80-520v-80h144L52-772l56-56 172 172v-144h80v280H80Zm80 360q-33 0-56.5-23.5T80-240v-200h80v200h320v80H160Zm640-280v-280H440v-80h360q33 0 56.5 23.5T880-720v280h-80ZM560-160v-200h320v200H560Z"
                />
            </svg>
        </button>

        <button
            v-if="!(isWindowsPlatform && isPipEnabled)"
            class="icon-button icon-button--player icon-button--lg"
            :class="{ 'icon-button--disabled': isMacPlatform && isPipEnabled }"
            :disabled="isMacPlatform && isPipEnabled"
            :aria-disabled="isMacPlatform && isPipEnabled"
            @click="emit('toggle-fullscreen')"
        >
            <svg
                v-if="!isFullscreen"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="currentColor"
            >
                <path
                    d="M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z"
                ></path>
            </svg>
            <svg
                v-else
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="currentColor"
            >
                <path
                    d="M5 16h3v3h2v-5H5v2zm3-8H5v2h5V5H8v3zm6 11h2v-3h3v-2h-5v5zm2-11V5h-2v5h5V8h-3z"
                ></path>
            </svg>
        </button>
    </div>
</template>

<style scoped>
.icon-button--lg.pip-toggle svg {
    width: 26px !important;
    height: 26px !important;
}

.pip-toggle--active {
    color: #8fb3ff;
}

.track-menu__header-actions {
    display: inline-flex;
    align-items: center;
    gap: 8px;
}

.track-menu__mode-toggle {
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.92);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 8px;
    padding: 4px 6px;
    cursor: pointer;
    transition: background-color 0.2s ease;
}

.track-menu__mode-toggle:hover {
    background: rgba(255, 255, 255, 0.1);
}

.track-menu__mode-label {
    font-size: 12px;
    font-weight: 600;
}

.track-menu__mode-switch {
    width: 26px;
    height: 12px;
    background: rgba(255, 255, 255, 0.28);
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    padding: 2px;
    transition: background-color 0.2s ease;
}

.track-menu__mode-switch--on {
    background: #8fb3ff;
}

.track-menu__mode-thumb {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #fff;
    transform: translateX(0);
    transition: transform 0.2s ease;
}

.track-menu__mode-switch--on .track-menu__mode-thumb {
    transform: translateX(12px);
}

.track-menu__section + .track-menu__section {
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.track-menu__sub-targets {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 8px 14px 4px;
}

.track-menu__sub-target {
    height: 28px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.78);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    cursor: pointer;
    transition:
        background-color 0.2s ease,
        border-color 0.2s ease,
        color 0.2s ease;
}

.track-menu__sub-target:hover {
    background: rgba(255, 255, 255, 0.11);
}

.track-menu__sub-target--active {
    background: rgba(143, 179, 255, 0.22);
    border-color: rgba(143, 179, 255, 0.75);
    color: #dfeaff;
}

.track-menu__item--disabled {
    color: rgba(255, 255, 255, 0.36);
    cursor: not-allowed;
}

.track-menu__item--disabled:hover {
    background: transparent;
}

.track-menu__item--disabled .track-menu__check svg {
    opacity: 0.35;
}

.track-menu__section-label {
    padding: 8px 16px 4px;
    color: rgba(255, 255, 255, 0.66);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
}

.track-menu__footer {
    display: flex;
    flex-direction: column;
    gap: 12px;
}
</style>

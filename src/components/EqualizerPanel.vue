<script setup lang="ts">
import { ref } from "vue";
import {
    EQ_BAND_LABELS,
    EQ_GAIN_RANGE,
    EQ_PRESETS,
    NIGHT_MODE_LABELS,
    type AudioEnhancementsController,
    type NightModeLevel,
} from "../composables/useAudioEnhancements";
import ControlSlider from "./player-controls/ControlSlider.vue";

const props = defineProps<{
    audio: AudioEnhancementsController;
    visible: boolean;
}>();

const emit = defineEmits<{ (e: "close"): void }>();

const NIGHT_LEVELS: NightModeLevel[] = ["off", "light", "medium", "strong"];
const GAIN_STEPS = [100, 125, 150, 200] as const;

// --- vertical band sliders -------------------------------------------------
// Custom pointer handling rather than rotated <input type="range">, so the
// track geometry and the centre-out fill are straightforward.

const dragBand = ref<number | null>(null);

/** Slider travel in px; the fill and thumb are positioned as a 0..1 fraction. */
const gainToFraction = (gain: number) =>
    0.5 - gain / (EQ_GAIN_RANGE * 2);

const setFromClientY = (index: number, clientY: number, track: HTMLElement) => {
    const rect = track.getBoundingClientRect();
    if (rect.height <= 0) return;
    const fraction = Math.min(1, Math.max(0, (clientY - rect.top) / rect.height));
    props.audio.setEqBand(index, (0.5 - fraction) * EQ_GAIN_RANGE * 2);
};

const onBandPointerDown = (index: number, event: PointerEvent) => {
    const track = event.currentTarget as HTMLElement;
    dragBand.value = index;
    track.setPointerCapture(event.pointerId);
    setFromClientY(index, event.clientY, track);
};

const onBandPointerMove = (index: number, event: PointerEvent) => {
    if (dragBand.value !== index) return;
    setFromClientY(index, event.clientY, event.currentTarget as HTMLElement);
};

const onBandPointerUp = (event: PointerEvent) => {
    const track = event.currentTarget as HTMLElement;
    if (track.hasPointerCapture(event.pointerId)) {
        track.releasePointerCapture(event.pointerId);
    }
    dragBand.value = null;
};

/** Arrow keys nudge a band by 1 dB, so the EQ stays keyboard-reachable. */
const onBandKeydown = (index: number, event: KeyboardEvent) => {
    const current = props.audio.state.eqBands[index] ?? 0;
    if (event.key === "ArrowUp") {
        props.audio.setEqBand(index, current + 1);
    } else if (event.key === "ArrowDown") {
        props.audio.setEqBand(index, current - 1);
    } else if (event.key === "Home") {
        props.audio.setEqBand(index, 0);
    } else {
        return;
    }
    event.preventDefault();
    event.stopPropagation();
};

const formatGain = (gain: number) => {
    if (Math.abs(gain) < 0.05) return "0";
    return `${gain > 0 ? "+" : ""}${gain.toFixed(gain % 1 === 0 ? 0 : 1)}`;
};
</script>

<template>
    <transition name="eq-slide">
        <div v-if="visible" class="eq" data-window-no-drag>
            <div class="eq__header">
                <span class="eq__title">Audio</span>
                <button
                    class="eq__close"
                    type="button"
                    aria-label="Close audio panel"
                    @click="emit('close')"
                >
                    ✕
                </button>
            </div>

            <!-- Night mode -->
            <section class="eq__section">
                <div class="eq__row">
                    <span class="eq__heading">Night mode</span>
                    <div class="eq__seg">
                        <button
                            v-for="level in NIGHT_LEVELS"
                            :key="level"
                            class="eq__seg-btn"
                            :class="{
                                'eq__seg-btn--active':
                                    props.audio.state.nightMode === level,
                            }"
                            type="button"
                            @click="props.audio.setNightMode(level)"
                        >
                            {{ NIGHT_MODE_LABELS[level] }}
                        </button>
                    </div>
                </div>
                <p class="eq__hint">
                    Evens out loud action and quiet dialogue — for late-night or
                    laptop-speaker viewing.
                </p>
            </section>

            <!-- Dialogue + gain -->
            <section class="eq__section">
                <ControlSlider
                    label="Dialogue boost"
                    :value="props.audio.state.dialogueBoost"
                    :min="0"
                    :max="100"
                    :step="1"
                    unit="%"
                    :precision="0"
                    @change="props.audio.setDialogueBoost($event)"
                    @reset="props.audio.setDialogueBoost(0)"
                />
                <div class="eq__row">
                    <span class="eq__heading">Volume boost</span>
                    <div class="eq__seg">
                        <button
                            v-for="step in GAIN_STEPS"
                            :key="step"
                            class="eq__seg-btn"
                            :class="{
                                'eq__seg-btn--active':
                                    props.audio.state.gain === step,
                            }"
                            type="button"
                            @click="props.audio.setGain(step)"
                        >
                            {{ step }}%
                        </button>
                    </div>
                </div>
            </section>

            <!-- Equalizer -->
            <section class="eq__section">
                <div class="eq__row">
                    <span class="eq__heading">Equalizer</span>
                    <button
                        class="eq__toggle"
                        type="button"
                        role="switch"
                        :aria-checked="props.audio.state.eqEnabled"
                        @click="props.audio.setEqEnabled(!props.audio.state.eqEnabled)"
                    >
                        <span
                            class="eq__switch"
                            :class="{
                                'eq__switch--on': props.audio.state.eqEnabled,
                            }"
                        >
                            <span class="eq__thumb"></span>
                        </span>
                    </button>
                </div>

                <div
                    class="eq__strip"
                    :class="{ 'eq__strip--off': !props.audio.state.eqEnabled }"
                >
                    <div
                        v-for="(label, index) in EQ_BAND_LABELS"
                        :key="label"
                        class="eq__band"
                    >
                        <span class="eq__band-gain">
                            {{ formatGain(props.audio.state.eqBands[index] ?? 0) }}
                        </span>
                        <div
                            class="eq__band-track"
                            role="slider"
                            tabindex="0"
                            :aria-label="`${label} hertz`"
                            :aria-valuemin="-EQ_GAIN_RANGE"
                            :aria-valuemax="EQ_GAIN_RANGE"
                            :aria-valuenow="props.audio.state.eqBands[index] ?? 0"
                            @pointerdown.prevent="onBandPointerDown(index, $event)"
                            @pointermove="onBandPointerMove(index, $event)"
                            @pointerup="onBandPointerUp"
                            @pointercancel="onBandPointerUp"
                            @keydown="onBandKeydown(index, $event)"
                        >
                            <span class="eq__band-centre"></span>
                            <span
                                class="eq__band-fill"
                                :style="{
                                    top:
                                        Math.min(
                                            50,
                                            gainToFraction(
                                                props.audio.state.eqBands[index] ?? 0,
                                            ) * 100,
                                        ) + '%',
                                    height:
                                        Math.abs(
                                            gainToFraction(
                                                props.audio.state.eqBands[index] ?? 0,
                                            ) *
                                                100 -
                                                50,
                                        ) + '%',
                                }"
                            ></span>
                            <span
                                class="eq__band-thumb"
                                :style="{
                                    top:
                                        gainToFraction(
                                            props.audio.state.eqBands[index] ?? 0,
                                        ) *
                                            100 +
                                        '%',
                                }"
                            ></span>
                        </div>
                        <span class="eq__band-label">{{ label }}</span>
                    </div>
                </div>

                <div class="eq__presets">
                    <button
                        v-for="preset in EQ_PRESETS"
                        :key="preset.id"
                        class="eq__chip"
                        :class="{
                            'eq__chip--active':
                                props.audio.state.eqPreset === preset.id,
                        }"
                        type="button"
                        @click="props.audio.applyEqPreset(preset.id)"
                    >
                        {{ preset.name }}
                    </button>
                </div>
            </section>

            <button class="eq__reset" type="button" @click="props.audio.reset()">
                Reset all audio
            </button>
        </div>
    </transition>
</template>

<style scoped>
.eq {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 340px;
    max-width: 92vw;
    z-index: 115;
    display: flex;
    flex-direction: column;
    gap: 16px;
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

.eq__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.eq__title {
    font-size: 16px;
    font-weight: 700;
}

.eq__close {
    border: none;
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    cursor: pointer;
    font-size: 13px;
}

.eq__close:hover {
    background: rgba(255, 255, 255, 0.2);
}

.eq__section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-bottom: 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.eq__section:last-of-type {
    border-bottom: none;
}

.eq__row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
}

.eq__heading {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.62);
}

.eq__hint {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.45);
}

.eq__seg {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.07);
}

.eq__seg-btn {
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.7);
    padding: 4px 8px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s ease, color 0.15s ease;
}

.eq__seg-btn:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.1);
}

.eq__seg-btn--active {
    background: rgba(255, 255, 255, 0.9);
    color: #16181c;
}

.eq__toggle {
    border: none;
    background: transparent;
    padding: 0;
    cursor: pointer;
}

.eq__switch {
    display: block;
    width: 34px;
    height: 19px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.18);
    transition: background-color 0.18s ease;
    position: relative;
}

.eq__switch--on {
    background: var(--progress-color, #4a9eff);
}

.eq__thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.18s ease;
}

.eq__switch--on .eq__thumb {
    transform: translateX(15px);
}

/* --- band strip --- */

.eq__strip {
    display: flex;
    justify-content: space-between;
    gap: 2px;
    transition: opacity 0.18s ease;
}

.eq__strip--off {
    opacity: 0.4;
}

.eq__band {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    min-width: 0;
}

.eq__band-gain {
    font-size: 9.5px;
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.5);
}

.eq__band-track {
    position: relative;
    width: 100%;
    height: 116px;
    cursor: ns-resize;
    touch-action: none;
    border-radius: 4px;
}

/* The visible rail is narrower than the hit area so the band is easy to grab. */
.eq__band-track::before {
    content: "";
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 3px;
    transform: translateX(-50%);
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.14);
}

.eq__band-track:focus-visible {
    outline: 1px solid rgba(255, 255, 255, 0.5);
    outline-offset: 2px;
}

.eq__band-centre {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 9px;
    height: 1px;
    transform: translate(-50%, -50%);
    background: rgba(255, 255, 255, 0.28);
}

.eq__band-fill {
    position: absolute;
    left: 50%;
    width: 3px;
    transform: translateX(-50%);
    border-radius: 2px;
    background: var(--progress-color, #4a9eff);
}

.eq__band-thumb {
    position: absolute;
    left: 50%;
    width: 11px;
    height: 11px;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.5);
    pointer-events: none;
}

.eq__band-label {
    font-size: 9.5px;
    color: rgba(255, 255, 255, 0.45);
}

.eq__presets {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
}

.eq__chip {
    border: 1px solid rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.82);
    border-radius: 999px;
    padding: 5px 10px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s ease, border-color 0.15s ease;
}

.eq__chip:hover {
    background: rgba(255, 255, 255, 0.12);
}

.eq__chip--active {
    background: rgba(255, 255, 255, 0.9);
    border-color: transparent;
    color: #16181c;
}

.eq__reset {
    border: 1px solid rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.8);
    border-radius: 8px;
    padding: 8px 0;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
}

.eq__reset:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
}

.eq-slide-enter-active,
.eq-slide-leave-active {
    transition: transform 0.22s ease, opacity 0.22s ease;
}

.eq-slide-enter-from,
.eq-slide-leave-to {
    transform: translateX(16px);
    opacity: 0;
}
</style>

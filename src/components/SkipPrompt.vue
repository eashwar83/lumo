<script setup lang="ts">
import type { SkipMarkersController } from "../composables/useSkipMarkers";

// Floating "Skip Intro" / "Skip Credits" button, bottom-right over the video.
// Only rendered when the current folder has markers and playback is inside one
// of them — see useSkipMarkers.evaluate().

const props = defineProps<{ skip: SkipMarkersController }>();

const onSkip = () => {
    if (props.skip.promptKind.value === "intro") {
        void props.skip.skipIntro();
    } else {
        void props.skip.skipCredits();
    }
};
</script>

<template>
    <transition name="skip-prompt">
        <div
            v-if="props.skip.promptKind.value"
            class="skip-prompt"
            data-window-no-drag
        >
            <button class="skip-prompt__btn" type="button" @click="onSkip">
                {{
                    props.skip.promptKind.value === "intro"
                        ? "Skip Intro"
                        : "Skip Credits"
                }}
                <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                    <path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z" />
                </svg>
            </button>
            <button
                class="skip-prompt__dismiss"
                type="button"
                aria-label="Dismiss"
                @click="props.skip.dismissPrompt()"
            >
                ✕
            </button>
        </div>
    </transition>
</template>

<style scoped>
.skip-prompt {
    position: fixed;
    right: 26px;
    /* Sits clear of the player controls without depending on their height. */
    bottom: 96px;
    z-index: 110;
    display: flex;
    align-items: center;
    gap: 4px;
    user-select: none;
    -webkit-user-select: none;
}

.skip-prompt__btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border: 1px solid rgba(255, 255, 255, 0.22);
    border-radius: 9px;
    background: rgba(18, 20, 24, 0.9);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    color: #fff;
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45);
    transition: background-color 0.15s ease;
}

.skip-prompt__btn:hover {
    background: rgba(38, 42, 50, 0.95);
}

.skip-prompt__btn svg {
    width: 15px;
    height: 15px;
}

.skip-prompt__dismiss {
    width: 26px;
    height: 26px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 7px;
    background: rgba(18, 20, 24, 0.85);
    color: rgba(255, 255, 255, 0.7);
    font-size: 11px;
    cursor: pointer;
}

.skip-prompt__dismiss:hover {
    color: #fff;
    background: rgba(38, 42, 50, 0.95);
}

.skip-prompt-enter-active,
.skip-prompt-leave-active {
    transition: opacity 0.2s ease, transform 0.2s ease;
}

.skip-prompt-enter-from,
.skip-prompt-leave-to {
    opacity: 0;
    transform: translateY(8px);
}
</style>

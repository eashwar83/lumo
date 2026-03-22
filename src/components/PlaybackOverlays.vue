<script setup lang="ts">
type StatusOverlayMode = "play" | "pause";

defineProps<{
    isLoading: boolean;
    showStatusOverlay: boolean;
    statusOverlayMode: StatusOverlayMode;
}>();
</script>

<template>
    <div v-if="isLoading" class="loading-overlay ui-surface">
        <div class="loading-card">
            <div class="loading-spinner"></div>
            <div class="loading-text">Loading...</div>
        </div>
    </div>

    <transition name="fade-in">
        <div v-if="showStatusOverlay" class="status-overlay">
            <div
                class="status-overlay__button"
                :title="statusOverlayMode === 'pause' ? 'Paused' : 'Playing'"
                :aria-label="
                    statusOverlayMode === 'pause' ? 'Paused' : 'Playing'
                "
                role="img"
            >
                <svg
                    v-if="statusOverlayMode === 'play'"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path d="M8 5v14l11-7z" />
                </svg>
                <svg v-else viewBox="0 0 24 24" fill="currentColor">
                    <path d="M6 5h4v14H6V5zm8 0h4v14h-4V5z" />
                </svg>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.loading-overlay {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 140;
    pointer-events: none;
}

.status-overlay {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 130;
    pointer-events: none;
}

.status-overlay__button {
    width: 128px;
    height: 128px;
    border-radius: 999px;
    border: none;
    background: radial-gradient(
        circle,
        rgba(0, 0, 0, 0.55) 0%,
        rgba(0, 0, 0, 0.4) 60%,
        rgba(0, 0, 0, 0.28) 100%
    );
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 18px 36px rgba(0, 0, 0, 0.4);
    overflow: hidden;
    pointer-events: none;
    transition:
        transform 0.2s ease,
        background-color 0.2s ease,
        box-shadow 0.2s ease;
}

.status-overlay__button svg {
    width: 82px;
    height: 82px;
    display: block;
}

.fade-in-enter-active,
.fade-in-leave-active {
    transition: opacity 0.2s ease;
}

.fade-in-enter-from,
.fade-in-leave-to {
    opacity: 0;
}

.loading-card {
    width: 120px;
    height: 120px;
    padding: 0;
    border-radius: 14px;
    background: rgba(18, 18, 18, 0.6);
    color: #f6f6f6;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    box-shadow: 0 16px 32px rgba(0, 0, 0, 0.35);
    backdrop-filter: blur(10px);
}

.loading-spinner {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: rgba(255, 255, 255, 0.9);
    animation: loading-spin 0.9s linear infinite;
}

.loading-text {
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
}

:root[data-theme="light"] .loading-card {
    background: rgba(255, 255, 255, 0.9);
    color: rgba(28, 38, 52, 0.9);
    border: 1px solid rgba(0, 0, 0, 0.12);
    box-shadow:
        0 14px 28px rgba(0, 0, 0, 0.16),
        inset 0 1px 0 rgba(255, 255, 255, 0.7);
}

:root[data-theme="light"] .loading-spinner {
    border-color: rgba(0, 0, 0, 0.2);
    border-top-color: rgba(57, 108, 216, 0.78);
}

:root[data-theme="light"] .loading-text {
    color: rgba(31, 43, 59, 0.72);
}

:root[data-theme="graphite"] .loading-card {
    background: rgba(37, 42, 48, 0.9);
    color: #edf1f6;
    border: 1px solid rgba(146, 158, 175, 0.32);
    box-shadow:
        0 20px 36px rgba(0, 0, 0, 0.42),
        inset 0 1px 0 rgba(188, 196, 208, 0.08);
}

:root[data-theme="graphite"] .loading-spinner {
    border-color: rgba(188, 196, 208, 0.24);
    border-top-color: rgba(232, 238, 247, 0.92);
}

:root[data-theme="graphite"] .loading-text {
    color: rgba(221, 227, 236, 0.78);
}

@keyframes loading-spin {
    to {
        transform: rotate(360deg);
    }
}
</style>

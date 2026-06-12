<script setup lang="ts">
defineProps<{
    disabled: boolean;
}>();

const emit = defineEmits<{
    toggle: [];
}>();

const onToggle = () => {
    emit("toggle");
};
</script>

<template>
    <button
        class="playlist-peek-area"
        :class="{ 'playlist-peek-area--disabled': disabled }"
        type="button"
        title="Playlist"
        aria-label="Open playlist"
        @click="onToggle"
    >
        <span class="playlist-peek" aria-hidden="true">
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 -960 960 960"
                fill="currentColor"
            >
                <path d="M560-280 360-480l200-200v400Z" />
            </svg>
        </span>
    </button>
</template>

<style scoped>
.playlist-peek-area {
    position: fixed;
    top: var(--top-bar-height);
    right: 0;
    bottom: var(--controls-bar-height);
    width: var(--playlist-peek-reveal-width);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    z-index: 125;
    pointer-events: auto;
    border: none;
    padding: 0;
    background: transparent;
    cursor: pointer;
}

.playlist-peek-area--disabled {
    pointer-events: none;
}

.playlist-peek {
    margin-right: 12px;
    width: 38px;
    height: 60px;
    border-radius: 999px;
    border: 1px solid var(--glass-border);
    background: var(--glass-bg);
    color: var(--text-color);
    box-shadow:
        0 12px 24px rgba(0, 0, 0, 0.18),
        inset 0 1px 0 var(--glass-highlight);
    -webkit-backdrop-filter: blur(var(--glass-blur))
        saturate(var(--glass-saturate));
    backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transform: translateX(12px) translateY(var(--playlist-peek-offset-y));
    transition:
        opacity 0.2s ease,
        transform 0.2s ease,
        box-shadow 0.2s ease;
    pointer-events: auto;
}

.playlist-peek svg {
    width: 36px;
    height: 36px;
}

.playlist-peek-area:hover .playlist-peek,
.playlist-peek-area:focus-visible .playlist-peek {
    opacity: 1;
    transform: translateX(0) translateY(var(--playlist-peek-offset-y));
}

.playlist-peek-area:hover .playlist-peek {
    box-shadow: 0 16px 30px rgba(0, 0, 0, 0.22);
}

.playlist-peek-area:focus-visible {
    outline: 2px solid rgba(57, 108, 216, 0.6);
    outline-offset: 2px;
}

@media (prefers-color-scheme: dark) {
    :root:not([data-theme]) .playlist-peek {
        border-color: rgba(255, 255, 255, 0.18);
        color: #f6f6f6;
        box-shadow: 0 16px 30px rgba(0, 0, 0, 0.5);
    }

    :root:not([data-theme]) .playlist-peek:focus-visible {
        outline-color: rgba(143, 179, 255, 0.7);
    }
}

:root[data-theme="dark"] .playlist-peek,
:root[data-theme="graphite"] .playlist-peek {
    border-color: rgba(255, 255, 255, 0.18);
    color: #f6f6f6;
    box-shadow: 0 16px 30px rgba(0, 0, 0, 0.5);
}

:root[data-theme="dark"] .playlist-peek:focus-visible,
:root[data-theme="graphite"] .playlist-peek:focus-visible {
    outline-color: rgba(143, 179, 255, 0.7);
}
</style>

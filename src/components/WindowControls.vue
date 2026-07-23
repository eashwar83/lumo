<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Minimise / maximise / close for the custom (compact) title bar. Shared by the
// menu bar and the player header so there is one implementation of the button
// behaviour and the maximised-state tracking.

const isWindowMaximized = ref(false);

const syncMaximized = async () => {
    try {
        isWindowMaximized.value = await getCurrentWindow().isMaximized();
    } catch {
        // Ignore in non-Tauri environments.
    }
};

const onMinimize = async () => {
    try {
        await getCurrentWindow().minimize();
    } catch {
        // Ignore in non-Tauri environments.
    }
};

const onToggleMaximize = async () => {
    try {
        const currentWindow = getCurrentWindow();
        if (isWindowMaximized.value) {
            await currentWindow.unmaximize();
            isWindowMaximized.value = false;
        } else {
            await currentWindow.maximize();
            isWindowMaximized.value = true;
        }
    } catch {
        // Ignore in non-Tauri environments.
    }
};

const onClose = async () => {
    try {
        await getCurrentWindow().close();
    } catch {
        // Ignore in non-Tauri environments.
    }
};

// The window can also be maximised by dragging or by a shortcut, so track it.
let unlistenResized: (() => void) | null = null;

onMounted(async () => {
    void syncMaximized();
    try {
        unlistenResized = await getCurrentWindow().onResized(() => {
            void syncMaximized();
        });
    } catch {
        // Ignore in non-Tauri environments.
    }
});

onBeforeUnmount(() => {
    unlistenResized?.();
    unlistenResized = null;
});
</script>

<template>
    <div class="win-controls" data-window-no-drag>
        <button
            class="win-controls__btn"
            type="button"
            title="Minimize"
            aria-label="Minimize window"
            @click.stop="onMinimize"
        >
            <svg viewBox="0 0 10 10" aria-hidden="true">
                <line
                    x1="1.6"
                    y1="5.3"
                    x2="8.4"
                    y2="5.3"
                    stroke="currentColor"
                    stroke-width="1.2"
                    stroke-linecap="round"
                />
            </svg>
        </button>

        <button
            class="win-controls__btn"
            type="button"
            :title="isWindowMaximized ? 'Restore' : 'Maximize'"
            :aria-label="isWindowMaximized ? 'Restore window' : 'Maximize window'"
            @click.stop="onToggleMaximize"
        >
            <svg v-if="!isWindowMaximized" viewBox="0 0 10 10" aria-hidden="true">
                <rect
                    x="1.6"
                    y="1.6"
                    width="6.8"
                    height="6.8"
                    rx="0.8"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.1"
                    stroke-linejoin="round"
                />
            </svg>
            <svg v-else viewBox="0 0 10 10" aria-hidden="true">
                <rect
                    x="3"
                    y="1.6"
                    width="5.4"
                    height="5.4"
                    rx="0.7"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1"
                    stroke-linejoin="round"
                />
                <rect
                    x="1.6"
                    y="3"
                    width="5.4"
                    height="5.4"
                    rx="0.7"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1"
                    stroke-linejoin="round"
                />
            </svg>
        </button>

        <button
            class="win-controls__btn win-controls__btn--close"
            type="button"
            title="Close"
            aria-label="Close window"
            @click.stop="onClose"
        >
            <svg viewBox="0 0 10 10" aria-hidden="true">
                <line
                    x1="2.1"
                    y1="2.1"
                    x2="7.9"
                    y2="7.9"
                    stroke="currentColor"
                    stroke-width="1.2"
                    stroke-linecap="round"
                />
                <line
                    x1="7.9"
                    y1="2.1"
                    x2="2.1"
                    y2="7.9"
                    stroke="currentColor"
                    stroke-width="1.2"
                    stroke-linecap="round"
                />
            </svg>
        </button>
    </div>
</template>

<style scoped>
.win-controls {
    display: inline-flex;
    align-items: stretch;
    height: 100%;
}

.win-controls__btn {
    width: 44px;
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.85);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: default;
    transition: background-color 0.12s ease, color 0.12s ease;
}

.win-controls__btn svg {
    width: 10px;
    height: 10px;
}

.win-controls__btn:hover {
    background: rgba(255, 255, 255, 0.14);
    color: #fff;
}

/* Windows convention: close turns red, not grey. */
.win-controls__btn--close:hover {
    background: #e81123;
    color: #fff;
}

:root[data-theme="light"] .win-controls__btn {
    color: rgba(33, 44, 57, 0.85);
}

:root[data-theme="light"] .win-controls__btn:hover {
    background: rgba(0, 0, 0, 0.08);
    color: #101418;
}

:root[data-theme="light"] .win-controls__btn--close:hover {
    background: #e81123;
    color: #fff;
}
</style>

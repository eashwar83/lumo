<script setup lang="ts">
type SideActionId = "home" | "history" | "network" | "settings";

const props = defineProps<{
    isPlaybackActive: boolean;
    activePanel: SideActionId;
    navActivePanel: SideActionId | null;
}>();

const emit = defineEmits<{
    navigate: [panel: SideActionId];
}>();

const getTitle = (panel: SideActionId) => {
    const panelName = panel[0].toUpperCase() + panel.slice(1);
    return panelName;
};

const onNavigate = (panel: SideActionId) => {
    emit("navigate", panel);
};
</script>

<template>
    <nav class="side-actions ui-surface" aria-label="Primary">
        <button
            class="side-actions__btn"
            :class="{ 'side-actions__btn--active': navActivePanel === 'home' }"
            type="button"
            :title="getTitle('home')"
            :aria-label="getTitle('home')"
            @click="onNavigate('home')"
        >
            <svg
                class="side-actions__icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
            >
                <path d="M3 10.5L12 3l9 7.5" />
                <path d="M5 10v10h14V10" />
                <path d="M9 20v-6h6v6" />
            </svg>
        </button>
        <button
            class="side-actions__btn"
            :class="{
                'side-actions__btn--active': navActivePanel === 'history',
            }"
            type="button"
            :title="getTitle('history')"
            :aria-label="getTitle('history')"
            @click="onNavigate('history')"
        >
            <svg
                class="side-actions__icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
            >
                <circle cx="12" cy="12" r="9" />
                <path d="M12 7v6l4 2" />
            </svg>
        </button>
        <button
            class="side-actions__btn"
            :class="{
                'side-actions__btn--active': navActivePanel === 'network',
            }"
            type="button"
            :title="getTitle('network')"
            :aria-label="getTitle('network')"
            @click="onNavigate('network')"
        >
            <svg
                class="side-actions__icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
            >
                <circle cx="12" cy="12" r="9" />
                <path d="M2 12h20" />
                <path d="M12 3c3 3.5 3 14.5 0 18" />
                <path d="M12 3c-3 3.5-3 14.5 0 18" />
            </svg>
        </button>
        <button
            class="side-actions__btn"
            :class="{
                'side-actions__btn--active': navActivePanel === 'settings',
            }"
            type="button"
            :title="getTitle('settings')"
            :aria-label="getTitle('settings')"
            @click="onNavigate('settings')"
        >
            <svg
                class="side-actions__icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
            >
                <path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z" />
                <path d="M19.4 15a7.9 7.9 0 0 0 .1-6" />
                <path d="M4.5 9a7.9 7.9 0 0 0 .1 6" />
                <path d="M6.6 5.6a9.2 9.2 0 0 1 10.8 0" />
                <path d="M6.6 18.4a9.2 9.2 0 0 0 10.8 0" />
            </svg>
        </button>
    </nav>
</template>

<style scoped>
.side-actions {
    position: fixed;
    left: 24px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    flex-direction: column;
    z-index: 120;
    background: rgba(255, 255, 255, 0.6);
    border: 1px solid rgba(0, 0, 0, 0.15);
    border-radius: 14px;
    overflow: hidden;
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.12);
}

.side-actions__btn {
    border: none;
    background: transparent;
    color: var(--text-color);
    padding: 12px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 10px;
    transition: background-color 0.2s ease;
    white-space: nowrap;
    position: relative;
}

.side-actions__btn + .side-actions__btn {
    border-top: 1px solid rgba(0, 0, 0, 0.12);
}

.side-actions__btn:hover {
    background: rgba(0, 0, 0, 0.14);
}

.side-actions__btn--active {
    background: rgba(0, 0, 0, 0.08);
}

.side-actions__btn--active .side-actions__icon {
    transform: scale(1.05);
}

.side-actions__icon {
    width: 20px;
    height: 20px;
    flex: 0 0 auto;
}

@media (prefers-color-scheme: dark) {
    :root:not([data-theme]) .side-actions {
        background: rgba(20, 20, 20, 0.5);
        border-color: rgba(255, 255, 255, 0.2);
        box-shadow: 0 8px 20px rgba(0, 0, 0, 0.45);
    }

    :root:not([data-theme]) .side-actions__btn {
        color: #f6f6f6;
    }

    :root:not([data-theme]) .side-actions__btn + .side-actions__btn {
        border-top-color: rgba(255, 255, 255, 0.12);
    }

    :root:not([data-theme]) .side-actions__btn:hover {
        background: rgba(255, 255, 255, 0.16);
    }

    :root:not([data-theme]) .side-actions__btn--active {
        background: rgba(255, 255, 255, 0.12);
    }
}

:root[data-theme="dark"] .side-actions,
:root[data-theme="graphite"] .side-actions {
    background: rgba(20, 20, 20, 0.5);
    border-color: rgba(255, 255, 255, 0.2);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.45);
}

:root[data-theme="dark"] .side-actions__btn,
:root[data-theme="graphite"] .side-actions__btn {
    color: #f6f6f6;
}

:root[data-theme="dark"] .side-actions__btn + .side-actions__btn,
:root[data-theme="graphite"] .side-actions__btn + .side-actions__btn {
    border-top-color: rgba(255, 255, 255, 0.12);
}

:root[data-theme="dark"] .side-actions__btn:hover,
:root[data-theme="graphite"] .side-actions__btn:hover {
    background: rgba(255, 255, 255, 0.16);
}

:root[data-theme="dark"] .side-actions__btn--active,
:root[data-theme="graphite"] .side-actions__btn--active {
    background: rgba(255, 255, 255, 0.12);
}
</style>

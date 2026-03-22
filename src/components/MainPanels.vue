<script setup lang="ts">
import type { HistoryEntry } from "../types/history";
import type { NetworkPlayRequest } from "../types/network";
import HomePanel from "../panels/HomePanel.vue";
import HistoryPanel from "../panels/HistoryPanel.vue";
import NetworkPanel from "../panels/NetworkPanel.vue";
import SettingsPanel from "../panels/SettingsPanel.vue";

const props = defineProps<{
    isFileLoaded: boolean;
    hover: boolean;
    history: HistoryEntry[];
    historyReady: boolean;
    hideHistory: boolean;
    mode: "home" | "history" | "network" | "settings";
}>();

const emit = defineEmits<{
    (e: "update:hover", value: boolean): void;
    (e: "open-file-picker"): void;
    (e: "play-history", entry: HistoryEntry): void;
    (e: "play-network", payload: NetworkPlayRequest): void;
    (e: "clear-history"): void;
    (e: "remove-history", entry: HistoryEntry): void;
    (e: "toggle-pin-history", entry: HistoryEntry): void;
}>();

const showPanels = () => !props.isFileLoaded;
</script>

<template>
    <div class="main-panels ui-surface">
        <div
            :class="[
                'main-panels__content',
                { 'main-panels__content--history': props.mode === 'history' },
            ]"
        >
            <HomePanel
                v-if="props.mode === 'home' && showPanels()"
                :is-file-loaded="props.isFileLoaded"
                :hover="props.hover"
                @open-file-picker="emit('open-file-picker')"
                @update:hover="emit('update:hover', $event)"
            />

            <HistoryPanel
                v-if="
                    showPanels() &&
                    props.mode === 'history' &&
                    !props.hideHistory
                "
                :history="props.history"
                :is-loading="!props.historyReady"
                @play-history="emit('play-history', $event)"
                @clear-history="emit('clear-history')"
                @remove-history="emit('remove-history', $event)"
                @toggle-pin-history="emit('toggle-pin-history', $event)"
            />

            <NetworkPanel
                v-if="showPanels() && props.mode === 'network'"
                @play-network="emit('play-network', $event)"
            />

            <SettingsPanel v-if="showPanels() && props.mode === 'settings'" />
        </div>
    </div>
</template>

<style scoped>
.main-panels {
    position: fixed;
    top: var(--top-bar-height);
    left: var(--side-actions-space);
    right: var(--side-actions-space);
    bottom: 0;
    display: flex;
    justify-content: center;
    align-items: center;
    width: auto;
    height: auto;
    box-sizing: border-box;
    min-height: 0;
    z-index: 10;
    pointer-events: none;
    /* background-color: green; */
}

.main-panels__content {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 16px;
    pointer-events: auto;
}

.main-panels__content--history {
    padding-top: 0;
}
</style>

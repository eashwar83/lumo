<script setup lang="ts">
import type { HistoryEntry } from "../types/history";
import type { NetworkPlayRequest } from "../types/network";
import type { FavoriteFolder, PlaylistEntry } from "../types/playlist";
import HomePanel from "../panels/HomePanel.vue";
import HistoryPanel from "../panels/HistoryPanel.vue";
import FavoritesPanel from "../panels/FavoritesPanel.vue";
import NetworkPanel from "../panels/NetworkPanel.vue";
import YouTubePanel from "../panels/YouTubePanel.vue";

const props = defineProps<{
    isFileLoaded: boolean;
    hover: boolean;
    history: HistoryEntry[];
    historyReady: boolean;
    hideHistory: boolean;
    favorites: PlaylistEntry[];
    favoriteFolders: FavoriteFolder[];
    favoritesByFolder: Record<string, PlaylistEntry[]>;
    favoriteFolderCounts: Record<string, number>;
    activeFavoriteFolderId: string | null;
    mode: "home" | "history" | "favorites" | "youtube" | "network" | "settings";
    currentUrl: string;
}>();

const emit = defineEmits<{
    (e: "update:hover", value: boolean): void;
    (e: "open-file-picker"): void;
    (e: "play-history", entry: HistoryEntry): void;
    (e: "play-network", payload: NetworkPlayRequest): void;
    (e: "clear-history"): void;
    (e: "remove-history", entry: HistoryEntry): void;
    (e: "toggle-pin-history", entry: HistoryEntry): void;
    (e: "play-favorite", entry: PlaylistEntry): void;
    (e: "remove-favorite", entry: PlaylistEntry): void;
    (e: "clear-favorites"): void;
    (e: "select-favorite-folder", id: string | null): void;
    (e: "create-favorite-folder", name: string): void;
    (e: "rename-favorite-folder", payload: { id: string; name: string }): void;
    (e: "delete-favorite-folder", id: string): void;
    (e: "move-favorite-to-folder", payload: { path: string; folderId: string }): void;
    (e: "move-many-favorites", payload: { paths: string[]; folderId: string }): void;
    (e: "remove-many-favorites", paths: string[]): void;
    (e: "export-favorites"): void;
    (e: "import-favorites"): void;
    (e: "play-youtube", payload: { url: string; title?: string }): void;
    (e: "youtube-notify", message: string): void;
}>();

const showPanels = () => !props.isFileLoaded;
</script>

<template>
    <div class="main-panels ui-surface">
        <div
            :class="[
                'main-panels__content',
                { 'main-panels__content--history': props.mode === 'history' },
                {
                    'main-panels__content--aligned-panel':
                        props.mode === 'network' ||
                        props.mode === 'settings' ||
                        props.mode === 'favorites' ||
                        props.mode === 'youtube',
                },
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

            <FavoritesPanel
                v-if="showPanels() && props.mode === 'favorites'"
                :favorites="props.favorites"
                :folders="props.favoriteFolders"
                :favorites-by-folder="props.favoritesByFolder"
                :folder-counts="props.favoriteFolderCounts"
                :active-folder-id="props.activeFavoriteFolderId"
                @play="emit('play-favorite', $event)"
                @remove="emit('remove-favorite', $event)"
                @clear="emit('clear-favorites')"
                @select-folder="emit('select-favorite-folder', $event)"
                @create-folder="emit('create-favorite-folder', $event)"
                @rename-folder="emit('rename-favorite-folder', $event)"
                @delete-folder="emit('delete-favorite-folder', $event)"
                @move-to-folder="emit('move-favorite-to-folder', $event)"
                @move-many-to-folder="emit('move-many-favorites', $event)"
                @remove-many="emit('remove-many-favorites', $event)"
                @export="emit('export-favorites')"
                @import="emit('import-favorites')"
            />

            <YouTubePanel
                v-show="showPanels() && props.mode === 'youtube'"
                :is-visible="showPanels() && props.mode === 'youtube'"
                @play-youtube="emit('play-youtube', $event)"
                @notify="emit('youtube-notify', $event)"
            />

            <NetworkPanel
                v-show="showPanels() && props.mode === 'network'"
                :history="props.history"
                :current-url="props.currentUrl"
                :is-visible="showPanels() && props.mode === 'network'"
                @play-network="emit('play-network', $event)"
            />
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
    background: transparent;
    border-color: transparent;
    box-shadow: none;
    -webkit-backdrop-filter: none;
    backdrop-filter: none;
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

.main-panels__content--aligned-panel {
    justify-content: flex-start;
    padding-top: 6px;
}
</style>
